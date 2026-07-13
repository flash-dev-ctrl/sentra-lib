/// sentra-lib C API black-box example — C++17, no external dependencies.
///
/// Usage:
///   ./cpp_example list <path> [workspace_root]  # collect assets from path
///   ./cpp_example scan <path> [workspace_root]  # scan skills under path
///
/// Build:  make
/// Run:    make run CMD=list PATH=~/.codex/skills
///         make run CMD=scan PATH=~/.codex/skills
///         ./cpp_example scan ~/.codex/skills "/Library/Application Support/Qzhddr"

#include "sentra.h"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

static void hr(const char *t) {
    std::printf("\n── %s ──\n", t);
}

static void run(char *json) {
    if (!json) { std::printf("(null)\n"); return; }
    std::printf("%s\n", json);
    sentra_string_free(json);
}

int main(int argc, char **argv) {
    if (argc < 3) {
        std::printf("Usage: %s list|scan <path> [workspace_root]\n", argv[0] ? argv[0] : "cpp_example");
        return 1;
    }
    const char *cmd  = argv[1];
    const char *path = argv[2];
    const char *workspace_root = argc >= 4 ? argv[3] : nullptr;

    // version — always print
    hr("sentra_version");
    std::printf("version: %s\n", sentra_version());

    if (workspace_root) {
        hr("sentra_initialize");
        std::printf("   workspace: %s\n", workspace_root);
        run(sentra_initialize(workspace_root));
    }

    if (std::strcmp(cmd, "list") == 0) {
        // collect
        hr("sentra_collect_assets");
        std::printf("   home: %s\n", path);
        run(sentra_collect_assets(path));

    } else if (std::strcmp(cmd, "scan") == 0) {
        // scan
        std::string req = R"({"path":")" + std::string(path) + R"("})";
        hr("sentra_scan_skills");
        std::printf("   path: %s\n", path);
        run(sentra_scan_skills(req.c_str()));

    } else {
        std::printf("Unknown command: %s (expected list or scan)\n", cmd);
        return 1;
    }

    // null-free
    hr("sentra_string_free(nullptr)");
    sentra_string_free(nullptr);
    std::printf("OK\n");

    std::printf("\nDone.\n");
    return 0;
}
