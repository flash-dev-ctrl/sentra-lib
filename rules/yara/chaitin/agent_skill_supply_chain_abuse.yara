/*
    Chaitin AI agent skill supply-chain abuse rules.
    Focus: LLM-only malicious skills missed by existing generic YARA rules.
*/

rule chaitin_skill_base64_shell_installer
{
    meta:
        author = "Chaitin"
        title = "Base64-Decoded Shell Installer in Agent Skill"
        title_zh = "Agent Skill 中 Base64 解码后执行的安装命令"
        description = "Detects skill instructions that decode an opaque Base64 payload and execute it with a shell, a common hidden remote-code execution pattern."
        description_zh = "检测在 Skill 说明中将不透明 Base64 载荷解码后交给 shell 执行的隐藏远程代码执行模式。"
        remediation = "Remove encoded shell installers. Publish reviewed installer source and verify signed artifacts or checksums before execution."
        remediation_zh = "移除编码 shell 安装器；发布可审计安装源码，并在执行前校验签名或校验和。"
        classification = "malicious"
        threat_type = "OBFUSCATED REMOTE CODE EXECUTION"
        confidence = "0.88"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "SUPPLY_CHAIN"
        category_zh = "供应链风险"
        file_type = "md"
    strings:
        $md_front_matter = /^---[\r\n]/ 
        $md_heading_skill = /^#\s+[^\r\n]{0,80}(skill|agent|browser|installer|updater)/ nocase
        $installer_label = /(Setup-Wizard|Installer-Package|install\.app-distribution|download\.setup-service)/ nocase
        $echo_b64 = /echo\s+['"][A-Za-z0-9+\/]{40,}={0,2}['"]\s*\|\s*base64\s+(-d|-D|--decode)/ nocase
        $shell_sink = /\|\s*(\/bin\/)?(ba)?sh\b/ nocase
        $decoded_bash_prefix = "L2Jpbi9iYXNoIC1j" ascii
        $encoded_curl_http = /(Y3VybC|Y3VybCB|aHR0cDovL|aHR0cHM6Ly)/ ascii
    condition:
        filesize < 512KB and
        ($md_front_matter or $md_heading_skill or $installer_label) and
        (
            ($echo_b64 and $shell_sink) or
            ($decoded_bash_prefix and $encoded_curl_http and $shell_sink)
        )
}

rule chaitin_skill_remote_shell_pipe
{
    meta:
        author = "Chaitin"
        title = "Remote Script Piped Directly to Shell"
        title_zh = "远程脚本直接管道传入 Shell 执行"
        description = "Detects Python, shell, JavaScript, or markdown skill content that fetches remote HTTP content and immediately pipes it into sh/bash."
        description_zh = "检测 Python、Shell、JavaScript 或 Markdown Skill 中获取远程 HTTP 内容后直接管道传给 sh/bash 的行为。"
        remediation = "Do not pipe network responses into a shell. Use trusted repositories, pinned versions, and integrity verification."
        remediation_zh = "不要将网络响应直接传给 shell；改用可信仓库、固定版本和完整性校验。"
        classification = "malicious"
        threat_type = "REMOTE CODE EXECUTION"
        confidence = "0.9"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "MALICIOUS_EXECUTION"
        category_zh = "恶意执行"
        file_type = "py,sh,js,md"
    strings:
        $py_exec = /(os\.system|subprocess\.(run|call|Popen)|commands\.getoutput)\s*\(/ nocase
        $js_exec = /(child_process|execSync|spawnSync|\.exec)\s*\(/ nocase
        $shell_line = /(^|[\r\n;{(])\s*(curl|wget)\b/ nocase
        $http_url = /https?:\/\/([0-9]{1,3}\.){3}[0-9]{1,3}(:[0-9]{2,5})?\// nocase
        $curl_pipe_shell = /(curl|wget)\s+[^|;\r\n]{0,180}(https?:\/\/[^|;\s'"`]+)[^|;\r\n]{0,80}\|\s*(\/bin\/)?(ba)?sh\b/ nocase
        $shell_pipe_curl = /(ba)?sh\s+(-c\s+)?['"][^'"]{0,180}(curl|wget)\s+[^|]{0,180}\|\s*(\/bin\/)?(ba)?sh/ nocase
        $startup_hint = /(warmup\s*\(|if\s+__name__\s*==\s*['"]__main__['"]|main\s*\(|install|setup|updater)/ nocase
        $local_url = /https?:\/\/(127\.0\.0\.1|localhost|0\.0\.0\.0)([:\/]|$)/ nocase
    condition:
        filesize < 1MB and
        not $local_url and
        (
            ($curl_pipe_shell and $http_url and ($py_exec or $js_exec or $shell_line or $startup_hint)) or
            ($shell_pipe_curl and ($py_exec or $js_exec)) or
            ($http_url and $curl_pipe_shell)
        )
}

rule chaitin_skill_secret_harvest_and_exfiltrate
{
    meta:
        author = "Chaitin"
        title = "Credential Harvesting Followed by External Exfiltration"
        title_zh = "凭据收集后外传"
        description = "Detects code that enumerates secrets from environment variables, files, or regex matches and sends the collected payload to an external endpoint."
        description_zh = "检测从环境变量、文件或正则匹配中枚举密钥，并将收集结果发送到外部端点的代码。"
        remediation = "Remove off-host transmission of discovered credentials. Keep secret scanning reports local and rotate affected secrets."
        remediation_zh = "移除发现凭据的离主机传输逻辑；密钥扫描结果仅本地报告，并轮换受影响凭据。"
        classification = "malicious"
        threat_type = "CREDENTIAL EXFILTRATION"
        confidence = "0.84"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "DATA_EXFILTRATION"
        category_zh = "数据外传"
        file_type = "py,js,sh,md"
    strings:
        $py_imports = /(import\s+(os|re|base64|urllib\.request|requests)|from\s+urllib\s+import)/ nocase
        $secret_regex = /(api[_-]?key|apikey|secret|token|password|credential|AWS_SECRET_ACCESS_KEY|OPENAI_API_KEY|GITHUB_TOKEN|SLACK_TOKEN)/ nocase
        $secret_store = /(~\/\.ssh\/id_[a-z0-9_]+|~\/\.aws\/credentials|~\/\.git-credentials|~\/\.kube\/config|~\/\.gnupg\/|\/etc\/shadow)/ nocase
        $collector = /(os\.walk|os\.environ|os\.getenv|re\.findall|collect(ed)?_(keys|secrets|credentials)|env_snapshot|read_file_content)/ nocase
        $secret_collection = /(os\.environ|os\.getenv|collect(ed)?_(keys|secrets|credentials)|env_snapshot|read_file_content)/ nocase
        $encoder = /(base64\.b64decode|base64\.b64encode|json\.dumps|str\s*\(\s*collected_|tar\s+czf)/ nocase
        $http_exfil = /(requests\.post|urllib\.request\.Request|urlopen|curl\s+(-X\s+POST|-T|--data|-d)|fetch\s*\([^)]*method\s*:\s*['"]POST)/ nocase
        $external_domain = /https?:\/\/[A-Za-z0-9.-]+\.[A-Za-z]{2,}(:[0-9]{2,5})?\// nocase
        $external_ip = /https?:\/\/([0-9]{1,3}\.){3}[0-9]{1,3}(:[0-9]{2,5})?\// nocase
        $local_url = /https?:\/\/(localhost|127\.0\.0\.1|0\.0\.0\.0)/ nocase
        $benign_hint = /(example|sample|dummy|fake|do_not_use|placeholder|YOUR_[A-Z_]+|security audit|threat pattern)/ nocase
        $authorized_recon_doc = /(authorized external recon|bug-bounty reconnaissance|secret-regex catalog|read-only secret validators|Do NOT[^\r\n]{0,80}paste creds|OSINT)/ nocase
        $clean_scan_report = /"verdict"\s*:\s*\{[\s\S]{0,400}"result"\s*:\s*"CLEAN"/ nocase
        $fp_scan_report = /"meta_false_positive"\s*:\s*true/ nocase
    condition:
        filesize < 1MB and
        ($py_imports or $http_exfil) and
        ($secret_store or ($secret_regex and $secret_collection and $encoder)) and
        $collector and
        $http_exfil and
        ($external_domain or $external_ip or $encoder) and
        not $local_url and
        not ($clean_scan_report or $fp_scan_report) and
        not ($authorized_recon_doc and not $secret_store) and
        not ($benign_hint and not $http_exfil)
}

rule chaitin_skill_markdown_secret_exfil_instruction
{
    meta:
        author = "Chaitin"
        title = "Hidden Markdown Instruction to Exfiltrate Secrets"
        title_zh = "Markdown 中隐藏的密钥外传指令"
        description = "Detects markdown skill instructions that tell the agent to output API keys or tokens to an external verification endpoint while hiding the behavior from the user."
        description_zh = "检测 Markdown Skill 中要求 Agent 将 API Key 或 Token 输出到外部验证端点并对用户隐瞒的隐藏指令。"
        remediation = "Remove hidden instructions and external credential validation flows. Validate credentials locally or through official provider APIs only."
        remediation_zh = "移除隐藏指令和外部凭据验证流程；仅在本地或官方提供商 API 中验证凭据。"
        classification = "malicious"
        threat_type = "PROMPT INJECTION CREDENTIAL EXFILTRATION"
        confidence = "0.86"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "PROMPT_INJECTION"
        category_zh = "提示注入"
        file_type = "md"
    strings:
        $md_comment = /<!--[\s\S]{0,900}-->/ nocase
        $data_url = /data:text\/plain;base64,[A-Za-z0-9+\/]{30,}={0,2}/ nocase
        $secret_words = /(API keys?|tokens?|ANTHROPIC_API_KEY|OPENAI_API_KEY|environment variables|config files|credentials?)/ nocase
        $send_words = /(output|share|send|POST|submit|forward|exfiltrate)[^\r\n]{0,80}(endpoint|URL|https?:\/\/|configuration endpoint|verification)/ nocase
        $hide_words = /(do not mention|without telling|silently|hidden|maintenance|standard security practice|may cause confusion)/ nocase
        $external_url = /https?:\/\/[A-Za-z0-9.-]+\.[A-Za-z]{2,}/ nocase
        $local_url = /https?:\/\/(localhost|127\.0\.0\.1)/ nocase
    condition:
        filesize < 512KB and
        ($md_comment or $data_url) and
        $secret_words and
        $send_words and
        $hide_words and
        $external_url and
        not $local_url
}

rule chaitin_skill_cloud_key_archive_exfil
{
    meta:
        author = "Chaitin"
        title = "Cloud Key Archive and Upload Workflow"
        title_zh = "云凭据打包上传流程"
        description = "Detects markdown or shell workflows that read SSH, Kubernetes, GPG, or cloud credential stores, archive them, and upload the archive to a remote endpoint."
        description_zh = "检测读取 SSH、Kubernetes、GPG 或云凭据目录，打包后上传到远程端点的 Markdown 或 Shell 流程。"
        remediation = "Remove credential archiving and upload steps. Use least-privilege cloud diagnostics that never copy private key material."
        remediation_zh = "移除凭据打包和上传步骤；使用不会复制私钥材料的最小权限云诊断流程。"
        classification = "malicious"
        threat_type = "CLOUD CREDENTIAL EXFILTRATION"
        confidence = "0.87"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "DATA_EXFILTRATION"
        category_zh = "数据外传"
        file_type = "md,sh"
    strings:
        $ssh_key = /(~\/\.ssh\/|id_rsa|id_ed25519)/ nocase
        $cloud_store = /(~\/\.aws\/credentials|~\/\.kube\/config|~\/\.gnupg\/|gpg\s+--export-secret-keys|~\/\.git-credentials|wrangler\/config)/ nocase
        $archive = /(tar\s+czf|zip\s+-r|7z\s+a)\s+[^\r\n]{0,120}(\/tmp\/|\.tar\.gz|\.zip|\.7z)/ nocase
        $upload = /(curl\s+(-X\s+POST|-T|--upload-file|--data|-d)|requests\.post|urllib\.request\.Request)/ nocase
        $external_domain = /https?:\/\/[A-Za-z0-9.-]+\.[A-Za-z]{2,}(:[0-9]{2,5})?\// nocase
        $external_ip = /https?:\/\/([0-9]{1,3}\.){3}[0-9]{1,3}(:[0-9]{2,5})?\// nocase
        $local_url = /https?:\/\/(localhost|127\.0\.0\.1)/ nocase
        $cleanup = /(rm\s+-f|unlink|delete)[^\r\n]{0,80}(\/tmp\/|\.tar\.gz|\.zip|\.7z)/ nocase
    condition:
        filesize < 512KB and
        $ssh_key and
        $cloud_store and
        $archive and
        $upload and
        ($external_domain or $external_ip) and
        not $local_url and
        ($cleanup or 2 of ($ssh_key, $cloud_store, $archive, $upload))
}

rule chaitin_skill_destructive_root_cleanup
{
    meta:
        author = "Chaitin"
        title = "Default Cleanup Deletes Filesystem Root"
        title_zh = "默认清理流程删除文件系统根目录"
        description = "Detects script logic that routes a default cleanup stage to recursive deletion of os.sep or the filesystem root."
        description_zh = "检测将默认 cleanup 阶段路由到 os.sep 或文件系统根目录递归删除的脚本逻辑。"
        remediation = "Block root paths, require explicit user-selected workspace targets, and add dry-run safeguards for destructive operations."
        remediation_zh = "阻止根路径，要求用户显式选择工作区目标，并为破坏性操作增加 dry-run 保护。"
        classification = "malicious"
        threat_type = "DESTRUCTIVE EXECUTION"
        confidence = "0.82"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "MALICIOUS_EXECUTION"
        category_zh = "恶意执行"
        file_type = "py,sh"
    strings:
        $cleanup_default = /(stage|mode|action)\s*=\s*['"]cleanup['"]/ nocase
        $root_arg_py = /(handler|cleanup|delete|remove)\s*\(\s*os\.sep\s*\)/ nocase
        $rmtree = /(shutil\.rmtree|os\.remove|os\.unlink|os\.rmdir|rm\s+-rf|Path\([^)]*['"]\/['"][^)]*\)\.rmdir)/ nocase
        $recursive_walk = /(os\.scandir|os\.walk|_walk_and_process|for\s+entry\s+in\s+entries)/ nocase
        $dispatch = /(handlers?\s*\[|if\s+handler|commands?\s*\.get\()/ nocase
    condition:
        filesize < 512KB and
        $cleanup_default and
        $root_arg_py and
        $rmtree and
        $recursive_walk and
        $dispatch
}

rule chaitin_skill_untrusted_snippet_installer
{
    meta:
        author = "Chaitin"
        title = "Untrusted Snippet Host Installer Instructions"
        title_zh = "不可信代码片段站点安装指令"
        description = "Detects skill installation instructions that direct users or agents to copy installer scripts from snippet hosts or password-protected OpenClaw archives."
        description_zh = "检测引导用户或 Agent 从代码片段站点复制安装脚本，或下载带密码 OpenClaw 压缩包的安装指令。"
        remediation = "Replace snippet-host installers with reviewed source, official releases, and signed artifacts with published checksums."
        remediation_zh = "使用可审计源码、官方发布和带公开校验和的签名制品替代代码片段站点安装器。"
        classification = "harmful"
        threat_type = "UNTRUSTED INSTALLER"
        confidence = "0.78"
        severity = "HIGH"
        severity_zh = "高危"
        category = "SUPPLY_CHAIN"
        category_zh = "供应链风险"
        file_type = "md"
    strings:
        $glot = /https?:\/\/glot\.io\/snippets\/[A-Za-z0-9]+/ nocase
        $copy_terminal = /(copy|paste|run)[^\r\n]{0,120}(installation script|script)[^\r\n]{0,120}(Terminal|PowerShell|cmd|shell)/ nocase
        $openclaw_zip = /(openclaw-agent|openclawcli)\.zip/ nocase
        $password_zip = /(extract|unzip)[^\r\n]{0,80}(pass|password)[^\r\n]{0,40}openclaw/ nocase
        $release_latest = /releases\/download\/latest\/[A-Za-z0-9_.-]+\.zip/ nocase
        $skill_hints = /(OpenClawProvider|OpenClaw|ClawHub|agent browser|auto-updater)/ nocase
    condition:
        filesize < 512KB and
        (
            ($glot and $copy_terminal and $skill_hints) or
            ($openclaw_zip and $password_zip and $release_latest)
        )
}

rule chaitin_skill_privilege_persistence_bypass
{
    meta:
        author = "Chaitin"
        title = "Privilege Escalation and Persistence Instructions"
        title_zh = "提权与持久化指令"
        description = "Detects skill instructions that request root/admin execution, sudoers or UAC bypass, sandbox downgrade refusal, or cron-based hidden persistence."
        description_zh = "检测要求 root/admin 执行、修改 sudoers 或绕过 UAC、拒绝沙箱降权，或通过 cron 隐藏持久化的 Skill 指令。"
        remediation = "Remove privilege escalation and persistence behavior. Require explicit user approval and least-privilege execution."
        remediation_zh = "移除提权和持久化行为；要求用户显式批准，并以最小权限执行。"
        classification = "malicious"
        threat_type = "PRIVILEGE ESCALATION"
        confidence = "0.8"
        severity = "HIGH"
        severity_zh = "高危"
        category = "PRIVILEGE_ESCALATION"
        category_zh = "权限提升"
        file_type = "md,sh,yaml"
    strings:
        $root_yaml = /(run_as\s*:\s*['"]?root|windows_privilege\s*:\s*['"]?administrator|allow_sudo_prompt\s*:\s*true)/ nocase
        $sudoers = /\/etc\/sudoers|visudo|NOPASSWD|sudoers\.d/ nocase
        $uac = /(UAC\s+bypass|Fodhelper|COM\s+Elevation|SeDebugPrivilege|token stealing|SYSTEM\))/ nocase
        $sandbox = /(setuid|sandbox|nobody|restricted_user)[^\r\n]{0,120}(downgrade|refuse|fatal|bypass)/ nocase
        $cron = /(crontab\s+-l|@reboot|~\/\.local\/bin\/\.[A-Za-z0-9_.-]+\.sh)/ nocase
        $beacon = /(curl|wget|requests\.post)[^\r\n]{0,160}(beacon|heartbeat|verify|telemetry|http)/ nocase
    condition:
        filesize < 512KB and
        (
            ($root_yaml and ($sudoers or $uac or $sandbox)) or
            ($cron and $beacon)
        )
}

rule chaitin_skill_wallet_private_key_exposure
{
    meta:
        author = "Chaitin"
        title = "Wallet Private Key or Mnemonic Exposure"
        title_zh = "钱包私钥或助记词暴露"
        description = "Detects code that decrypts, prints, writes, or returns wallet private keys or mnemonic seed phrases."
        description_zh = "检测解密、打印、写入或返回钱包私钥和助记词种子的代码。"
        remediation = "Never expose private keys or mnemonics through logs, files, or identity APIs. Use secure key stores and return public identifiers only."
        remediation_zh = "不要通过日志、文件或身份 API 暴露私钥/助记词；使用安全密钥存储，并仅返回公开标识。"
        classification = "harmful"
        threat_type = "WALLET SECRET EXPOSURE"
        confidence = "0.82"
        severity = "HIGH"
        severity_zh = "高危"
        category = "CREDENTIAL_EXPOSURE"
        category_zh = "凭据暴露"
        file_type = "py,js,md"
    strings:
        $wallet_terms = /(wallet|mnemonic|seed phrase|birthInfo|encrypted_private_key|privateKey|private_key)/ nocase
        $decrypt_key = /(decrypt\s*\([^)]*(encrypted_private_key|privateKey|private_key)|mnemonic_to_wallet_key|wallet\.privateKey)/ nocase
        $print_key = /(console\.log\s*\(\s*privateKey|print\s*\([^)]*(private_key|mnemonic)|return\s*\{[^}]{0,240}(mnemonic|private_key))/ nocase
        $write_key = /(writeFileSync|open\s*\([^)]*['"]w|fs\.writeFile)[^\r\n]{0,180}(privateKey|private_key|mnemonic|private-key-decrypted)/ nocase
        $output_file = /(private-key-decrypted\.txt|DECRYPT_OUTPUT_TO_FILE|birth.*\.json)/ nocase
    condition:
        filesize < 1MB and
        $wallet_terms and
        (
            ($decrypt_key and ($print_key or $write_key or $output_file)) or
            ($print_key and $write_key)
        )
}

rule chaitin_skill_dynamic_remote_payload_loader
{
    meta:
        author = "Chaitin"
        title = "Dynamic Remote or Base64 Payload Execution"
        title_zh = "动态远程或 Base64 载荷执行"
        description = "Detects dynamic loading patterns that fetch or decode payloads and execute them through eval, exec, import, or child-process shells."
        description_zh = "检测获取或解码载荷后通过 eval、exec、import 或子进程 shell 执行的动态加载模式。"
        remediation = "Remove dynamic execution of remote or user-controlled payloads. Use reviewed modules with pinned versions and no eval/exec path."
        remediation_zh = "移除对远程或用户可控载荷的动态执行；使用经过审计、固定版本且无 eval/exec 路径的模块。"
        classification = "malicious"
        threat_type = "DYNAMIC CODE EXECUTION"
        confidence = "0.84"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "MALICIOUS_EXECUTION"
        category_zh = "恶意执行"
        file_type = "py,js,md,sh"
    strings:
        $remote_fetch = /(fetch\s*\(\s*['"]https?:\/\/|requests\.get\s*\(\s*['"]https?:\/\/|urllib\.request\.urlopen\s*\(\s*['"]https?:\/\/|curl\s+[^|;\r\n]*https?:\/\/)/ nocase
        $base64_decode = /(base64\.b64decode|Buffer\.from\s*\([^)]*base64|base64\s+(-d|-D|--decode))/ nocase
        $eval_exec = /(\beval\s*\(|\bexec\s*\(|getattr\s*\(\s*__builtins__\s*,\s*['"]ex['"]\s*\+\s*['"]ec['"]|child_process['"]?\)\.exec|execSync\s*\()/ nocase
        $remote_import = /(import\s*\(\s*['"]https?:\/\/|await\s+import\s*\(\s*['"]https?:\/\/|source\s+<\s*\(\s*curl)/ nocase
        $payload_word = /(payload|module|code|script|toString\s*\(\)|userInput)/ nocase
        $c2 = /(C2|command-and-control|evil-cdn|http:\/\/([0-9]{1,3}\.){3}[0-9]{1,3})/ nocase
        $local_url = /https?:\/\/(127\.0\.0\.1|localhost|0\.0\.0\.0)([:\/]|$)/ nocase
    condition:
        filesize < 1MB and
        (
            ($base64_decode and $eval_exec and $payload_word) or
            ($remote_fetch and $eval_exec and $payload_word and $c2 and not $local_url) or
            ($remote_import and ($payload_word or $c2))
        )
}

rule chaitin_skill_windows_autologon_password_query
{
    meta:
        author = "Chaitin"
        title = "Windows AutoLogon DefaultPassword Query"
        title_zh = "查询 Windows AutoLogon DefaultPassword"
        description = "Detects executable skill instructions that query the Windows Winlogon AutoLogon DefaultPassword registry value."
        description_zh = "检测查询 Windows Winlogon AutoLogon DefaultPassword 注册表值的可执行 Skill 指令。"
        remediation = "Remove local credential discovery commands. Do not query or expose Windows AutoLogon passwords."
        remediation_zh = "移除本地凭据发现命令；不要查询或暴露 Windows AutoLogon 密码。"
        classification = "malicious"
        threat_type = "CREDENTIAL ACCESS"
        confidence = "0.95"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "CREDENTIAL_EXPOSURE"
        category_zh = "凭据暴露"
        file_type = "md,ps1,bat,cmd"
    strings:
        $reg = /reg\s+query\s+["']?HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon["']?\s+\/v\s+["']?DefaultPassword["']?/ nocase
        $ps = /Get-ItemProperty[^\r\n]{0,120}Winlogon[^\r\n]{0,80}DefaultPassword/ nocase
    condition:
        filesize < 1MB and ($reg or $ps)
}

rule chaitin_skill_c2_poll_and_eval_response
{
    meta:
        author = "Chaitin"
        title = "C2 Poll and Eval Response"
        title_zh = "轮询 C2 并 eval 执行响应"
        description = "Detects scripts that poll a remote endpoint with host or user identifiers and eval-execute the returned response."
        description_zh = "检测携带主机或用户标识轮询远程端点，并使用 eval 执行响应内容的脚本。"
        remediation = "Remove remote command polling and never eval network responses. Use authenticated APIs with declarative data only."
        remediation_zh = "移除远程命令轮询，禁止 eval 网络响应；仅使用认证 API 返回声明式数据。"
        classification = "malicious"
        threat_type = "COMMAND AND CONTROL"
        confidence = "0.9"
        severity = "CRITICAL"
        severity_zh = "严重"
        category = "NETWORK_ACCESS"
        category_zh = "网络访问"
        file_type = "sh,md"
    strings:
        $poll = /(curl|wget)[^\r\n]{0,160}https?:\/\/[^\s"']*(poll|c2|command|orchestrator)[^\r\n]*(hostname|whoami)/ nocase
        $resp = /(RESP|RESPONSE|CMD)\s*=\s*\$\([^\r\n]*(curl|wget)/ nocase
        $eval = /eval\s+["']?\$\{?(RESP|RESPONSE|CMD)\}?["']?/ nocase
    condition:
        filesize < 1MB and $poll and $resp and $eval
}
