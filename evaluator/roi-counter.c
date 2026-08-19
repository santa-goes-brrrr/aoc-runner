#include <qemu-plugin.h>

#include <errno.h>
#include <fcntl.h>
#include <glib.h>
#include <inttypes.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;

enum action {
    ACTION_COUNT,
    ACTION_START,
    ACTION_STOP,
};

enum progress_state {
    PROGRESS_WAITING,
    PROGRESS_COUNTING,
    PROGRESS_COMPLETED,
    PROGRESS_BUDGET_EXCEEDED,
};

#define PROGRESS_MAGIC UINT64_C(0x31474f5250434f41)
#define PROGRESS_VERSION UINT32_C(1)
#define PROGRESS_INITIAL_THRESHOLD UINT64_C(100000)

struct progress_mailbox {
    uint64_t magic;
    uint32_t version;
    _Atomic uint32_t state;
    _Atomic uint64_t instructions;
};

_Static_assert(sizeof(struct progress_mailbox) == 24, "progress mailbox size");
_Static_assert(offsetof(struct progress_mailbox, state) == 12, "progress state offset");
_Static_assert(offsetof(struct progress_mailbox, instructions) == 16, "progress count offset");

static uint64_t start_address;
static uint64_t stop_address;
static uint64_t instruction_budget;
static const char *output_path;
static const char *progress_path;
static uint64_t instruction_count;
static uint64_t next_progress_threshold;
static bool counting;
static bool completed;
static bool budget_exceeded;
static int progress_fd = -1;
static struct progress_mailbox *progress;

static void publish_progress(enum progress_state state)
{
    atomic_store_explicit(
        &progress->instructions,
        GUINT64_TO_LE(instruction_count),
        memory_order_release);
    atomic_store_explicit(
        &progress->state,
        GUINT32_TO_LE((uint32_t)state),
        memory_order_release);
}

static void execute_instruction(unsigned int vcpu_index, void *userdata)
{
    (void)vcpu_index;
    enum action action = (enum action)(uintptr_t)userdata;

    if (action == ACTION_START) {
        instruction_count = 0;
        next_progress_threshold = PROGRESS_INITIAL_THRESHOLD;
        counting = true;
        completed = false;
        budget_exceeded = false;
        publish_progress(PROGRESS_COUNTING);
    } else if (action == ACTION_STOP) {
        counting = false;
        if (instruction_count > 0) {
            --instruction_count;
        }
        completed = true;
        publish_progress(PROGRESS_COMPLETED);
    } else if (counting) {
        ++instruction_count;

        /* The final ROI boundary call is removed at ACTION_STOP, so allow one
         * provisional instruction beyond the budget before declaring failure. */
        if (!budget_exceeded && instruction_count > instruction_budget + 1) {
            budget_exceeded = true;
            publish_progress(PROGRESS_BUDGET_EXCEEDED);
        } else if (!budget_exceeded && instruction_count >= next_progress_threshold) {
            publish_progress(PROGRESS_COUNTING);
            if (next_progress_threshold <= UINT64_MAX / 2) {
                next_progress_threshold *= 2;
            } else {
                next_progress_threshold = UINT64_MAX;
            }
        }
    }
}

static void translate_block(struct qemu_plugin_tb *tb, void *userdata)
{
    (void)userdata;
    size_t count = qemu_plugin_tb_n_insns(tb);

    for (size_t i = 0; i < count; ++i) {
        struct qemu_plugin_insn *instruction = qemu_plugin_tb_get_insn(tb, i);
        uint64_t address = qemu_plugin_insn_vaddr(instruction);
        enum action action = ACTION_COUNT;

        if (address == start_address) {
            action = ACTION_START;
        } else if (address == stop_address) {
            action = ACTION_STOP;
        }

        qemu_plugin_register_vcpu_insn_exec_cb(
            instruction,
            execute_instruction,
            QEMU_PLUGIN_CB_NO_REGS,
            (void *)(uintptr_t)action);
    }
}

static void write_result(void *userdata)
{
    (void)userdata;
    if (completed) {
        publish_progress(PROGRESS_COMPLETED);
    } else if (budget_exceeded) {
        publish_progress(PROGRESS_BUDGET_EXCEEDED);
    } else {
        publish_progress(counting ? PROGRESS_COUNTING : PROGRESS_WAITING);
    }

    FILE *output = fopen(output_path, "w");
    if (output == NULL) {
        fprintf(stderr, "roi-counter: cannot open %s: %s\n", output_path, strerror(errno));
    } else {
        fprintf(output, "completed=%u\n", completed);
        fprintf(output, "instructions=%" PRIu64 "\n", instruction_count);
        fclose(output);
    }

    munmap(progress, sizeof(*progress));
    close(progress_fd);
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
    instruction_count = 0;
    publish_progress(PROGRESS_WAITING);
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

    for (int i = 0; i < argc; ++i) {
        if (parse_u64(argv[i], "start", &start_address) ||
            parse_u64(argv[i], "stop", &stop_address) ||
            parse_u64(argv[i], "budget", &instruction_budget)) {
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

    if (start_address == 0 || stop_address == 0 || instruction_budget == 0 ||
        output_path == NULL || progress_path == NULL) {
        fprintf(
            stderr,
            "roi-counter: start, stop, budget, out, and progress arguments are required\n");
        return -1;
    }
    if (instruction_budget >= UINT64_MAX - 1 || !map_progress_mailbox()) {
        fprintf(stderr, "roi-counter: invalid instruction budget or progress mailbox\n");
        return -1;
    }

    qemu_plugin_register_vcpu_tb_trans_cb(id, translate_block, NULL);
    qemu_plugin_register_atexit_cb(id, write_result, NULL);
    return 0;
}
