/* nanosleep, sigfillset, and pthread_sigmask under strict ISO C. */
#define _POSIX_C_SOURCE 200809L

#include <qemu-plugin.h>

#include <errno.h>
#include <fcntl.h>
#include <glib.h>
#include <inttypes.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;

enum marker {
    MARKER_START = 1,
    MARKER_STOP = 2,
};

/* Every state from PROGRESS_BUDGET_EXCEEDED on is a failure and is final. */
enum progress_state {
    PROGRESS_WAITING,
    PROGRESS_COUNTING,
    PROGRESS_COMPLETED,
    PROGRESS_BUDGET_EXCEEDED,
    PROGRESS_STARTUP_EXCEEDED,
    PROGRESS_SHUTDOWN_EXCEEDED,
    PROGRESS_MARKER_VIOLATION,
};

static const char *const state_names[] = {
    [PROGRESS_WAITING] = "waiting",
    [PROGRESS_COUNTING] = "counting",
    [PROGRESS_COMPLETED] = "completed",
    [PROGRESS_BUDGET_EXCEEDED] = "budget-exceeded",
    [PROGRESS_STARTUP_EXCEEDED] = "startup-limit-exceeded",
    [PROGRESS_SHUTDOWN_EXCEEDED] = "shutdown-limit-exceeded",
    [PROGRESS_MARKER_VIOLATION] = "marker-violation",
};

#define PROGRESS_MAGIC UINT64_C(0x31474f5250434f41)
#define PROGRESS_VERSION UINT32_C(2)

/* Instructions are added to a per-vCPU counter by inline TCG code, one add per
 * translation block. A callback folds that counter into `flushed`, applies the
 * limits, and publishes progress every FLUSH_INTERVAL instructions. */
#define FLUSH_INTERVAL UINT64_C(1048576)

/* A block is credited in full when it is entered, so a count sampled between
 * the markers can run ahead of the truth by less than one block. QEMU caps a
 * block at 512 instructions; limits checked mid-run allow this much slack and
 * the exact comparison happens when a marker is reached. */
#define BLOCK_SLACK UINT64_C(4096)

/* Instructions the runtime may execute between the stop marker and shutdown. */
#define SHUTDOWN_LIMIT UINT64_C(1000000)

struct progress_mailbox {
    uint64_t magic;
    uint32_t version;
    _Atomic uint32_t state;
    _Atomic uint64_t instructions; /* inside the ROI */
    _Atomic uint64_t executed;     /* since reset; advances while the guest runs */
};

_Static_assert(sizeof(struct progress_mailbox) == 32, "progress mailbox size");
_Static_assert(offsetof(struct progress_mailbox, state) == 12, "progress state offset");
_Static_assert(offsetof(struct progress_mailbox, instructions) == 16, "progress count offset");
_Static_assert(offsetof(struct progress_mailbox, executed) == 24, "progress executed offset");

static uint64_t start_address;
static uint64_t stop_address;
static uint64_t instruction_budget;
static uint64_t startup_limit;
static uint64_t parent_process;
static const char *output_path;
static const char *progress_path;

static qemu_plugin_u64 pending;
static uint64_t flushed;
static uint64_t roi_origin;           /* instructions executed through the start marker */
static uint64_t roi_end;              /* instructions executed through the stop marker */
static uint64_t roi_instructions;
static uint64_t startup_instructions; /* reset vector through the start marker */
static enum progress_state state;
static int progress_fd = -1;
static struct progress_mailbox *progress;

static void fail(enum progress_state reason)
{
    if (state < PROGRESS_BUDGET_EXCEEDED) {
        state = reason;
    }
}

static void publish_progress(uint64_t executed)
{
    atomic_store_explicit(&progress->executed, GUINT64_TO_LE(executed), memory_order_release);
    atomic_store_explicit(
        &progress->instructions,
        GUINT64_TO_LE(roi_instructions),
        memory_order_release);
    atomic_store_explicit(&progress->state, GUINT32_TO_LE((uint32_t)state), memory_order_release);
}

/* Catches guests that never reach the next marker. `executed` may include up
 * to one block of instructions that have not run yet, hence BLOCK_SLACK. */
static void check_running_limits(uint64_t executed)
{
    switch (state) {
    case PROGRESS_WAITING:
        startup_instructions = executed;
        if (executed > startup_limit + BLOCK_SLACK) {
            fail(PROGRESS_STARTUP_EXCEEDED);
        }
        break;
    case PROGRESS_COUNTING:
        roi_instructions = executed - roi_origin;
        if (roi_instructions > instruction_budget + BLOCK_SLACK) {
            fail(PROGRESS_BUDGET_EXCEEDED);
        }
        break;
    case PROGRESS_COMPLETED:
        if (executed - roi_end > SHUTDOWN_LIMIT) {
            fail(PROGRESS_SHUTDOWN_EXCEEDED);
        }
        break;
    default:
        break;
    }
}

static void flush_pending(unsigned int vcpu_index, void *userdata)
{
    (void)userdata;
    flushed += qemu_plugin_u64_get(pending, vcpu_index);
    qemu_plugin_u64_set(pending, vcpu_index, 0);
    check_running_limits(flushed);
    publish_progress(flushed);
}

/* Each marker is honoured exactly once and in order. Executing one again would
 * otherwise let a guest restart or cut short its own measurement. */
static void reach_marker(unsigned int vcpu_index, void *userdata)
{
    uintptr_t packed = (uintptr_t)userdata;
    enum marker marker = (enum marker)(packed & 3);
    /* The block was credited in full on entry; `unexecuted` instructions of
     * it, starting with the marker, have not run yet. */
    uint64_t unexecuted = packed >> 2;
    uint64_t before = flushed + qemu_plugin_u64_get(pending, vcpu_index) - unexecuted;

    if (marker == MARKER_START && state == PROGRESS_WAITING) {
        startup_instructions = before + 1;
        if (startup_instructions > startup_limit) {
            fail(PROGRESS_STARTUP_EXCEEDED);
        } else {
            roi_origin = startup_instructions;
            roi_instructions = 0;
            state = PROGRESS_COUNTING;
        }
    } else if (marker == MARKER_STOP && state == PROGRESS_COUNTING) {
        /* The call that reached the stop marker is not part of the score. */
        uint64_t since_start = before - roi_origin;
        roi_instructions = since_start > 0 ? since_start - 1 : 0;
        if (roi_instructions > instruction_budget) {
            fail(PROGRESS_BUDGET_EXCEEDED);
        } else {
            roi_end = before + 1;
            state = PROGRESS_COMPLETED;
        }
    } else {
        fail(PROGRESS_MARKER_VIOLATION);
    }
    publish_progress(before);
}

static void translate_block(struct qemu_plugin_tb *tb, void *userdata)
{
    (void)userdata;
    size_t count = qemu_plugin_tb_n_insns(tb);

    qemu_plugin_register_vcpu_tb_exec_inline_per_vcpu(
        tb,
        QEMU_PLUGIN_INLINE_ADD_U64,
        pending,
        count);
    qemu_plugin_register_vcpu_tb_exec_cond_cb(
        tb,
        flush_pending,
        QEMU_PLUGIN_CB_NO_REGS,
        QEMU_PLUGIN_COND_GE,
        pending,
        FLUSH_INTERVAL,
        NULL);

    for (size_t i = 0; i < count; ++i) {
        struct qemu_plugin_insn *instruction = qemu_plugin_tb_get_insn(tb, i);
        uint64_t address = qemu_plugin_insn_vaddr(instruction);
        uintptr_t marker = 0;

        if (address == start_address) {
            marker = MARKER_START;
        } else if (address == stop_address) {
            marker = MARKER_STOP;
        }
        if (marker != 0) {
            qemu_plugin_register_vcpu_insn_exec_cb(
                instruction,
                reach_marker,
                QEMU_PLUGIN_CB_NO_REGS,
                (void *)((((uintptr_t)(count - i)) << 2) | marker));
        }
    }
}

static void write_result(void *userdata)
{
    (void)userdata;
    uint64_t executed = flushed + qemu_plugin_u64_get(pending, 0);
    if (state == PROGRESS_WAITING) {
        startup_instructions = executed;
    }
    publish_progress(executed);

    FILE *output = fopen(output_path, "w");
    if (output == NULL) {
        fprintf(stderr, "roi-counter: cannot open %s: %s\n", output_path, strerror(errno));
    } else {
        fprintf(output, "state=%s\n", state_names[state]);
        fprintf(output, "instructions=%" PRIu64 "\n", roi_instructions);
        fprintf(output, "startup_instructions=%" PRIu64 "\n", startup_instructions);
        fclose(output);
    }

    munmap(progress, sizeof(*progress));
    close(progress_fd);
}

/* QEMU must not outlive the evaluator that launched it: an orphan would keep
 * a CPU busy for as long as its guest runs. */
static void *watch_parent(void *userdata)
{
    (void)userdata;
    const struct timespec interval = {.tv_sec = 0, .tv_nsec = 100000000};

    while ((uint64_t)getppid() == parent_process) {
        nanosleep(&interval, NULL);
    }
    _exit(EXIT_FAILURE);
}

static bool start_parent_watchdog(void)
{
    /* The thread must not take signals meant for QEMU's own threads. */
    sigset_t all;
    sigset_t previous;
    pthread_t thread;

    sigfillset(&all);
    pthread_sigmask(SIG_BLOCK, &all, &previous);
    int error = pthread_create(&thread, NULL, watch_parent, NULL);
    pthread_sigmask(SIG_SETMASK, &previous, NULL);
    if (error != 0) {
        fprintf(stderr, "roi-counter: cannot start parent watchdog: %s\n", strerror(error));
        return false;
    }
    pthread_detach(thread);
    return true;
}

static bool parse_u64(const char *argument, const char *name, uint64_t *result)
{
    size_t name_length = strlen(name);
    if (strncmp(argument, name, name_length) != 0 || argument[name_length] != '=') {
        return false;
    }

    char *end = NULL;
    errno = 0;
    unsigned long long value = strtoull(argument + name_length + 1, &end, 0);
    if (errno != 0 || end == argument + name_length + 1 || *end != '\0') {
        return false;
    }

    *result = (uint64_t)value;
    return true;
}

static bool map_progress_mailbox(void)
{
    progress_fd = open(progress_path, O_RDWR);
    if (progress_fd < 0) {
        fprintf(stderr, "roi-counter: cannot open %s: %s\n", progress_path, strerror(errno));
        return false;
    }

    struct stat status;
    if (fstat(progress_fd, &status) != 0 || status.st_size < (off_t)sizeof(*progress)) {
        fprintf(stderr, "roi-counter: invalid progress mailbox %s\n", progress_path);
        close(progress_fd);
        progress_fd = -1;
        return false;
    }

    progress = mmap(
        NULL,
        sizeof(*progress),
        PROT_READ | PROT_WRITE,
        MAP_SHARED,
        progress_fd,
        0);
    if (progress == MAP_FAILED) {
        fprintf(stderr, "roi-counter: cannot map %s: %s\n", progress_path, strerror(errno));
        progress = NULL;
        close(progress_fd);
        progress_fd = -1;
        return false;
    }

    progress->magic = GUINT64_TO_LE(PROGRESS_MAGIC);
    progress->version = GUINT32_TO_LE(PROGRESS_VERSION);
    publish_progress(0);
    return true;
}

QEMU_PLUGIN_EXPORT int qemu_plugin_install(
    qemu_plugin_id_t id,
    const qemu_info_t *info,
    int argc,
    char **argv)
{
    if (!info->system_emulation || strcmp(info->target_name, "riscv64") != 0) {
        fprintf(stderr, "roi-counter: requires riscv64 system emulation\n");
        return -1;
    }
    if (info->system.smp_vcpus != 1 || info->system.max_vcpus != 1) {
        fprintf(stderr, "roi-counter: requires exactly one vCPU\n");
        return -1;
    }

    for (int i = 0; i < argc; ++i) {
        if (parse_u64(argv[i], "start", &start_address) ||
            parse_u64(argv[i], "stop", &stop_address) ||
            parse_u64(argv[i], "budget", &instruction_budget) ||
            parse_u64(argv[i], "startup_limit", &startup_limit) ||
            parse_u64(argv[i], "parent", &parent_process)) {
            continue;
        }
        if (strncmp(argv[i], "out=", 4) == 0 && argv[i][4] != '\0') {
            output_path = argv[i] + 4;
            continue;
        }
        if (strncmp(argv[i], "progress=", 9) == 0 && argv[i][9] != '\0') {
            progress_path = argv[i] + 9;
            continue;
        }
        fprintf(stderr, "roi-counter: unknown or invalid argument: %s\n", argv[i]);
        return -1;
    }

    if (start_address == 0 || stop_address == 0 || start_address == stop_address ||
        instruction_budget == 0 || startup_limit == 0 || output_path == NULL ||
        progress_path == NULL) {
        fprintf(
            stderr,
            "roi-counter: distinct start and stop, budget, startup_limit, out, and progress "
            "arguments are required\n");
        return -1;
    }
    if (instruction_budget >= UINT64_MAX - BLOCK_SLACK ||
        startup_limit >= UINT64_MAX - BLOCK_SLACK) {
        fprintf(stderr, "roi-counter: instruction limits are too large\n");
        return -1;
    }
    if (!map_progress_mailbox()) {
        return -1;
    }
    if (parent_process != 0 && !start_parent_watchdog()) {
        return -1;
    }

    pending = qemu_plugin_scoreboard_u64(qemu_plugin_scoreboard_new(sizeof(uint64_t)));
    qemu_plugin_register_vcpu_tb_trans_cb(id, translate_block, NULL);
    qemu_plugin_register_atexit_cb(id, write_result, NULL);
    return 0;
}
