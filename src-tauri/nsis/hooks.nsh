; ============================================================================
; TPlayer NSIS Installer Hooks
; 接入点：tauri.conf.json > bundle > windows > nsis > installerHooks
; 本文件在 Tauri 生成的 installer.nsi 顶部被 !include，
; 通过 NSIS_HOOK_* 宏名在固定位置展开（未定义的宏由 !ifmacrodef 自动跳过）：
;   NSIS_HOOK_PREINSTALL —— Section Install 的第一步（写入任何新版文件之前）
;
; PREINSTALL 职责（安装前旧版本检测与完整卸载）：
;   1. 结束旧版进程（TPlayer.exe / TPlayerNext.exe / tplayer-next.exe）
;   2. 枚举 HKCU 与 HKLM(64/32 位视图) 的卸载注册表，凡 DisplayName 含
;      "TPlayer"（不区分大小写，PotPlayer 显式排除）的条目均视为旧版本
;   3. 按卸载器类型静默卸载：NSIS(/S _?=) / Inno(/VERYSILENT) / MSI(/qn)
;   4. 卸载失败：交互模式弹窗由用户选择「继续安装(强制清理残留)」或「终止」；
;      静默模式(/S)自动强制清理后继续
;   5. 兜底清理：残留卸载注册表键、残留安装目录（仅路径含 tplayer 才允许删除）、
;      残留开始菜单/桌面快捷方式
;   6. 全程不触碰用户数据目录（%APPDATA%\com.tplayer* 等），升级后数据完整保留
;
; 说明：检测面向整个 TPlayer 产品族（旧版 TPlayer 与 TPlayerNext），与版本号
;       无关 —— 后续版本迭代无需修改本文件即可复用同一升级流程。
; 注意：本文件需保存为 UTF-8 with BOM（含中文文案，makensis 依赖 BOM 识别编码）。
; ============================================================================

!define TPN_UNINST_ROOT "Software\Microsoft\Windows\CurrentVersion\Uninstall"
; 命中判定：DisplayName 以 "tplayer" **开头**（大小写不敏感），覆盖
; "TPlayer" / "TPlayer Next" / "TPlayerNext" / "TPlayer 2.0.x" 等本产品族；
; 旧实现用任意位置子串匹配，会把 "MyTPlayer Tools" 这类第三方软件误判为旧版
; 并触发静默卸载。精确前缀匹配天然排除 PotPlayer 等仅含子串的软件。
!define TPN_NAME_PREFIX "tplayer"
!define TPN_PREFIX_LEN  7
; 安装目录守卫：要求路径含 "\tplayer" 目录段（排除 mytplayer 等），
; 命中 "…\TPlayer" 与 "…\TPlayerNext" 两种安装布局。
!define TPN_DIR_MARK "\tplayer"

; ---------------------------------------------------------------------------
; 按注册表视图枚举一个根键下的卸载子键并处理命中项。
; 用法：!insertmacro TPN_ScanHive "HKCU" "none" "a"
;       !insertmacro TPN_ScanHive "HKLM" "64"  "b"
;       !insertmacro TPN_ScanHive "HKLM" "32"  "c"
; 寄存器：$R1 枚举索引 / $R2 子键名 / $R3 DisplayName / $R4 小写 DisplayName
;         $R5 匹配临时 / $R6 当前根键（传给 TPN_ProcessEntry）
; 卸载会改动注册表，命中处理后索引归零重新枚举（已卸载的键不再命中，保证收敛）。
; ---------------------------------------------------------------------------
!macro TPN_ScanHive ROOT VIEW UID
  !if "${VIEW}" != "none"
    SetRegView ${VIEW}
  !endif
  StrCpy $R1 0
tpn_scan_${UID}_loop:
  EnumRegKey $R2 ${ROOT} "${TPN_UNINST_ROOT}" $R1
  StrCmp $R2 "" tpn_scan_${UID}_done
  ClearErrors
  ReadRegStr $R3 ${ROOT} "${TPN_UNINST_ROOT}\$R2" "DisplayName"
  IfErrors tpn_scan_${UID}_next
  ${StrCase} $R4 $R3 "L"
  ; 前缀精确匹配（见 TPN_NAME_PREFIX 注释）：仅本产品族命中
  StrCpy $R5 $R4 ${TPN_PREFIX_LEN}
  StrCmp $R5 "${TPN_NAME_PREFIX}" 0 tpn_scan_${UID}_next
  StrCpy $R6 "${ROOT}"
  Call TPN_ProcessEntry
  StrCmp $R9 "1" tpn_scan_${UID}_done
  StrCpy $R1 0
  Goto tpn_scan_${UID}_loop
tpn_scan_${UID}_next:
  IntOp $R1 $R1 + 1
  Goto tpn_scan_${UID}_loop
tpn_scan_${UID}_done:
  !if "${VIEW}" != "none"
    SetRegView lastused
  !endif
!macroend

; ---------------------------------------------------------------------------
; PREINSTALL hook 入口
; ---------------------------------------------------------------------------
!macro NSIS_HOOK_PREINSTALL
  DetailPrint "正在检测旧版本 TPlayer / TPlayerNext ..."
  Call TPN_CleanupOldVersions
  Pop $0
  ${If} $0 == "1"
    DetailPrint "用户选择终止安装。"
    Abort "已按您的选择终止安装。"
  ${EndIf}
!macroend

; ---------------------------------------------------------------------------
; 主流程：结束进程 -> 逐根键扫描并卸载旧版本 -> 返回终止标志
; 返回（栈顶）：$R9（"0"=继续安装 / "1"=用户要求终止）
; ---------------------------------------------------------------------------
Function TPN_CleanupOldVersions
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  Push $R5
  Push $R6
  Push $R7
  Push $R8
  ; 注意：$R9 作为“用户要求终止”标志在 TPN_* 函数间共享，不做保存/恢复

  StrCpy $R9 "0"
  Call TPN_KillOldProcesses

  !insertmacro TPN_ScanHive "HKCU" "none" "a"
  ${If} $R9 == "0"
    !insertmacro TPN_ScanHive "HKLM" "64" "b"
  ${EndIf}
  ${If} $R9 == "0"
    !insertmacro TPN_ScanHive "HKLM" "32" "c"
  ${EndIf}

  Pop $R8
  Pop $R7
  Pop $R6
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
  Push $R9
FunctionEnd

; ---------------------------------------------------------------------------
; 结束旧版本相关进程（释放文件句柄，避免卸载失败）
; ---------------------------------------------------------------------------
Function TPN_KillOldProcesses
  Push $R0
  nsExec::Exec 'taskkill /F /IM "TPlayer.exe" /T'
  Pop $R0
  nsExec::Exec 'taskkill /F /IM "TPlayerNext.exe" /T'
  Pop $R0
  nsExec::Exec 'taskkill /F /IM "tplayer-next.exe" /T'
  Pop $R0
  Sleep 800
  Pop $R0
FunctionEnd

; ---------------------------------------------------------------------------
; 处理一个命中的旧版本卸载条目
; 输入：$R2 = 卸载子键名  $R3 = DisplayName  $R6 = "HKCU"|"HKLM"
; 输出：$R9 = "1" 表示用户选择终止安装
; 内部约定：$R5 = InstallLocation（去引号）  $R2 = 卸载器 exe 路径
;           $R8 = 完整键路径  $R4 = 静默卸载命令
; ---------------------------------------------------------------------------
Function TPN_ProcessEntry
  Push $R0
  Push $R1
  Push $R4
  Push $R5
  Push $R7
  StrCpy $R8 "${TPN_UNINST_ROOT}\$R2"
  DetailPrint "发现旧版本：$R3（$R6）"

  ClearErrors
  ${If} $R6 == "HKCU"
    ReadRegStr $R7 HKCU "$R8" "UninstallString"
    ReadRegStr $R5 HKCU "$R8" "InstallLocation"
  ${Else}
    ReadRegStr $R7 HKLM "$R8" "UninstallString"
    ReadRegStr $R5 HKLM "$R8" "InstallLocation"
  ${EndIf}
  ${If} ${Errors}
    StrCpy $R7 ""
  ${EndIf}

  ; InstallLocation 去引号（Tauri 写入的是带引号形式）
  StrCpy $R0 $R5 1
  ${If} $R0 == '"'
    StrCpy $R5 $R5 "" 1
  ${EndIf}
  StrCpy $R0 $R5 1 -1
  ${If} $R0 == '"'
    StrLen $R0 $R5
    IntOp $R0 $R0 - 1
    StrCpy $R5 $R5 $R0
  ${EndIf}

  Call TPN_BuildSilentCommand   ; 入 $R7/$R5 -> 出 $R4 命令 / $R2 卸载器路径
  ${If} $R4 == ""
    Goto tpn_pe_failed
  ${EndIf}

  DetailPrint "正在静默卸载：$R3 ..."
  ClearErrors
  ExecWait "$R4" $R1
  ${If} ${Errors}
    Goto tpn_pe_failed
  ${EndIf}

  ; 等待卸载注册表键消失（最多约 10 秒）
  StrCpy $R0 0
tpn_pe_wait:
  Call TPN_KeyStillExists       ; 入 $R6/$R8 -> 出 $R1 "1"/"0"
  ${If} $R1 == "0"
    Goto tpn_pe_success
  ${EndIf}
  IntOp $R0 $R0 + 1
  IntCmp $R0 10 tpn_pe_failed 0 tpn_pe_failed
  Sleep 1000
  Goto tpn_pe_wait

tpn_pe_success:
  DetailPrint "旧版本卸载完成：$R3"
  Call TPN_CleanupShortcuts
  ; NSIS 卸载器就地运行无法删除自身，这里兜底清理残留目录
  Call TPN_RemoveLeftoverDir
  Goto tpn_pe_end

tpn_pe_failed:
  ${If} ${Silent}
    DetailPrint "旧版本自动卸载失败（静默安装，自动强制清理残留）：$R3"
    Call TPN_ForceCleanup
    Goto tpn_pe_end
  ${EndIf}
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_TOPMOST "检测到旧版本「$R3」，但自动卸载失败。$\n$\n选择「是」：强制清理其残留文件与注册表项，然后继续安装；$\n选择「否」：终止本次安装。$\n$\n(Failed to uninstall the old version. Continue installing anyway?)" IDYES tpn_pe_force IDNO tpn_pe_abort
tpn_pe_force:
  DetailPrint "按用户选择继续安装：强制清理旧版本残留：$R3"
  Call TPN_ForceCleanup
  Goto tpn_pe_end
tpn_pe_abort:
  StrCpy $R9 "1"

tpn_pe_end:
  Pop $R7
  Pop $R5
  Pop $R4
  Pop $R1
  Pop $R0
FunctionEnd

; ---------------------------------------------------------------------------
; 由 UninstallString 构造静默卸载命令
; 输入：$R7 = UninstallString 原文  $R5 = InstallLocation（可能为空）
; 输出：$R4 = 完整命令（"" = 无法解析）  $R2 = 卸载器 exe 路径（MSI 为空）
; ---------------------------------------------------------------------------
Function TPN_BuildSilentCommand
  Push $R0
  Push $R3
  StrCpy $R4 ""
  StrCpy $R2 ""
  ${StrCase} $R0 $R7 "L"

  ; --- MSI：MsiExec.exe /X{GUID} /qn ---
  ${StrLoc} $R3 $R0 "msiexec" ">"
  StrCmp $R3 "" tpn_bsc_notmsi
  ${StrLoc} $R3 $R0 "{" ">"
  StrCmp $R3 "" tpn_bsc_msi_noguid
  IntOp $R3 $R3 + 1
  StrCpy $R2 $R7 "" $R3
  ${StrLoc} $R3 $R2 "}" ">"
  StrCmp $R3 "" tpn_bsc_msi_noguid
  IntOp $R3 $R3 + 1
  StrCpy $R2 $R2 $R3
  StrCpy $R4 "MsiExec.exe /X$R2 /qn /norestart"
  StrCpy $R2 ""
  Goto tpn_bsc_done
tpn_bsc_msi_noguid:
  StrCpy $R4 "$R7 /qn /norestart"
  Goto tpn_bsc_done

tpn_bsc_notmsi:
  ; --- 提取 exe 路径 -> $R2 ---
  StrCpy $R2 $R7
  StrCpy $R3 $R7 1
  StrCmp $R3 '"' tpn_bsc_quoted
  ; 未加引号：取第一个空格之前的部分；该文件不存在则用整串
  ${StrLoc} $R3 $R7 " " ">"
  StrCmp $R3 "" tpn_bsc_gotpath
  StrCpy $R2 $R7 $R3
  IfFileExists "$R2" tpn_bsc_gotpath
  StrCpy $R2 $R7
  Goto tpn_bsc_gotpath
tpn_bsc_quoted:
  StrCpy $R2 $R7 "" 1
  StrCpy $R3 0
tpn_bsc_fq:
  StrCpy $R0 $R2 1 $R3
  StrCmp $R0 "" tpn_bsc_gotpath
  StrCmp $R0 '"' 0 tpn_bsc_fq_next
  StrCpy $R2 $R2 $R3
  Goto tpn_bsc_gotpath
tpn_bsc_fq_next:
  IntOp $R3 $R3 + 1
  Goto tpn_bsc_fq

tpn_bsc_gotpath:
  IfFileExists "$R2" 0 tpn_bsc_fail
  ${GetFileName} $R0 "$R2"
  ${StrCase} $R0 $R0 "L"
  ; --- NSIS 系卸载器 uninstall.exe：/S + _?= 就地运行，保证 ExecWait 同步等待 ---
  StrCmp $R0 "uninstall.exe" 0 tpn_bsc_tryinno
  StrCpy $R3 $R5
  ${If} $R3 == ""
    ${GetParent} $R3 "$R2"
  ${EndIf}
  StrCpy $R0 $R3 1
  ${If} $R0 == '"'
    StrCpy $R3 $R3 "" 1
  ${EndIf}
  StrCpy $R0 $R3 1 -1
  ${If} $R0 == '"'
    StrLen $R0 $R3
    IntOp $R0 $R0 - 1
    StrCpy $R3 $R3 $R0
  ${EndIf}
  StrCpy $R4 '"$R2" /S _?=$R3'
  Goto tpn_bsc_done
tpn_bsc_tryinno:
  ; --- Inno Setup：unins*.exe ---
  ${StrLoc} $R3 $R0 "unins" ">"
  StrCmp $R3 "" tpn_bsc_generic
  StrCpy $R4 '"$R2" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART'
  Goto tpn_bsc_done
tpn_bsc_generic:
  ; --- 未知类型：尽力静默 ---
  StrCpy $R4 '"$R2" /S'
  Goto tpn_bsc_done
tpn_bsc_fail:
  StrCpy $R4 ""
  StrCpy $R2 ""
tpn_bsc_done:
  Pop $R3
  Pop $R0
FunctionEnd

; ---------------------------------------------------------------------------
; 探测卸载注册表键是否仍存在
; 输入：$R6 = "HKCU"|"HKLM"  $R8 = 完整键路径
; 输出：$R1 = "1" 存在 / "0" 不存在
; ---------------------------------------------------------------------------
Function TPN_KeyStillExists
  ClearErrors
  ${If} $R6 == "HKCU"
    EnumRegKey $R1 HKCU "$R8" 0
  ${Else}
    EnumRegKey $R1 HKLM "$R8" 0
  ${EndIf}
  ${If} ${Errors}
    StrCpy $R1 "0"
  ${Else}
    StrCpy $R1 "1"
  ${EndIf}
FunctionEnd

; ---------------------------------------------------------------------------
; 强制清理残留：卸载注册表键 + 安装目录（带安全守卫）+ 快捷方式
; 输入：$R6 hive  $R8 键路径  $R5 InstallLocation  $R2 卸载器 exe 路径
; ---------------------------------------------------------------------------
Function TPN_ForceCleanup
  Push $R0
  Push $R1
  ; 1) 删除卸载注册表键
  ${If} $R6 == "HKCU"
    DeleteRegKey HKCU "$R8"
  ${Else}
    DeleteRegKey HKLM "$R8"
  ${EndIf}
  ; 2) 残留安装目录：仅当路径含 "\tplayer" 目录段、长度 >= 10 且不以 \ 结尾时才允许删除
  StrCpy $R1 $R5
  ${If} $R1 == ""
    ${If} $R2 != ""
      ${GetParent} $R1 "$R2"
    ${EndIf}
  ${EndIf}
  ${If} $R1 != ""
    ${StrCase} $R0 $R1 "L"
    ${StrLoc} $R0 $R0 "${TPN_DIR_MARK}" ">"
    ${If} $R0 != ""
      StrLen $R0 $R1
      IntCmp $R0 10 tpn_fc_rmdir tpn_fc_skipdir tpn_fc_rmdir
tpn_fc_rmdir:
      StrCpy $R0 $R1 1 -1
      ${If} $R0 == "\"
        Goto tpn_fc_skipdir
      ${EndIf}
      IfFileExists "$R1\*.*" 0 tpn_fc_skipdir
      DetailPrint "清理残留目录：$R1"
      RMDir /r "$R1"
tpn_fc_skipdir:
    ${EndIf}
  ${EndIf}
  ; 3) 残留快捷方式
  Call TPN_CleanupShortcuts
  Pop $R1
  Pop $R0
FunctionEnd

; ---------------------------------------------------------------------------
; 守卫式残留安装目录清理（成功卸载后的兜底 + 强制清理共用）
; 输入：$R5 = InstallLocation（可能为空）  $R2 = 卸载器 exe 路径
; 守卫条件：路径含 tplayer（不区分大小写）、长度 >= 10、不以 \ 结尾、目录存在
; ---------------------------------------------------------------------------
Function TPN_RemoveLeftoverDir
  Push $R0
  Push $R1
  StrCpy $R1 $R5
  ${If} $R1 == ""
    ${If} $R2 != ""
      ${GetParent} $R1 "$R2"
    ${EndIf}
  ${EndIf}
  ${If} $R1 != ""
    ${StrCase} $R0 $R1 "L"
    ${StrLoc} $R0 $R0 "tplayer" ">"
    ${If} $R0 != ""
      StrLen $R0 $R1
      IntCmp $R0 10 tpn_rld_rmdir tpn_rld_skip tpn_rld_rmdir
tpn_rld_rmdir:
      StrCpy $R0 $R1 1 -1
      ${If} $R0 == "\"
        Goto tpn_rld_skip
      ${EndIf}
      IfFileExists "$R1\*.*" 0 tpn_rld_skip
      DetailPrint "清理残留目录：$R1"
      RMDir /r "$R1"
tpn_rld_skip:
    ${EndIf}
  ${EndIf}
  Pop $R1
  Pop $R0
FunctionEnd

; ---------------------------------------------------------------------------
; 快捷方式兜底清理（旧卸载器通常已清理，此处为残留兜底，均为无副作用空操作）
; ---------------------------------------------------------------------------
Function TPN_CleanupShortcuts
  Push $R0
  Delete "$DESKTOP\TPlayer.lnk"
  Delete "$DESKTOP\TPlayerNext.lnk"
  RMDir /r "$SMPROGRAMS\TPlayer"
  RMDir /r "$SMPROGRAMS\TPlayerNext"
  Pop $R0
FunctionEnd
