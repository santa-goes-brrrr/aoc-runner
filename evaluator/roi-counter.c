#include <qemu-plugin.h>

#include <errno.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;

enum action {
    ACTION_COUNT,
    ACTION_START,
    ACTION_STOP,
};

static uint64_t start_address;
static uint64_t stop_address;
static const char *output_path;
static uint64_t instruction_count;
static bool counting;
static bool completed;

static void execute_instruction(unsigned int vcpu_index, void *userdata)
{
    (void)vcpu_index;
    enum action action = (enum action)(uintptr_t)userdata;

    if (action == ACTION_START) {
        instruction_count = 0;
        counting = true;
    } else if (action == ACTION_STOP) {
        counting = false;
        if (instruction_count > 0) {
            --instruction_count;
        }
        completed = true;
    } else if (counting) {
        ++instruction_count;
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
    FILE *output = fopen(output_path, "w");
    if (output == NULL) {
        fprintf(stderr, "roi-counter: cannot open %s: %s\n", output_path, strerror(errno));
        return;
    }

    fprintf(output, "completed=%u\n", completed);
    fprintf(output, "instructions=%" PRIu64 "\n", instruction_count);
    fclose(output);
}

static bool parse_address(const char *argument, const char *name, uint64_t *result)
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
        if (parse_address(argv[i], "start", &start_address) ||
            parse_address(argv[i], "stop", &stop_address)) {
            continue;
        }
        if (strncmp(argv[i], "out=", 4) == 0 && argv[i][4] != '\0') {
            output_path = argv[i] + 4;
            continue;
        }
        fprintf(stderr, "roi-counter: unknown or invalid argument: %s\n", argv[i]);
        return -1;
    }

    if (start_address == 0 || stop_address == 0 || output_path == NULL) {
        fprintf(stderr, "roi-counter: start, stop, and out arguments are required\n");
        return -1;
    }

    qemu_plugin_register_vcpu_tb_trans_cb(id, translate_block, NULL);
    qemu_plugin_register_atexit_cb(id, write_result, NULL);
    return 0;
}
