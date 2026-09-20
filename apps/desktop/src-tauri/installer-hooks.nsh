; Tauri NSIS 安装脚本钩子。
;
; 背景：installer.nsi 在复制文件前会通过 CheckIfAppIsRunning 检测应用是否
; 正在运行，运行中会弹出「确定(强制关闭) / 取消(中止安装)」提示框。
; 但当安装程序不处于前台（例如从运行中的应用内触发下载后安装、或被其他
; 窗口遮挡）时，该 MessageBox 可能显示在后面，看起来像安装程序「卡住」。
;
; 这里在安装 / 卸载开始前把安装程序窗口置于前台，确保提示框可见。
; 注：应用会在标签内容变化后 300ms 防抖自动保存会话（含未保存的编辑内容），
; 因此安装器强制结束进程不会丢失用户数据。

!macro NSIS_HOOK_PREINSTALL
  BringToFront
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  BringToFront
!macroend
