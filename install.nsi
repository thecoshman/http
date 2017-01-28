Name "http ${HTTP_VERSION}"
OutFile "http ${HTTP_VERSION} installer.exe"
LicenseData "LICENSE"
Icon "assets\favicon.ico"
ShowInstDetails show
InstallDir "$PROGRAMFILES\http"

Section
  SetOutPath $INSTDIR
  File /oname=http.exe "http-${HTTP_VERSION}.exe"
  WriteUninstaller "$INSTDIR\uninstall http ${HTTP_VERSION}.exe"
SectionEnd

Section "Update PATH"
  ${EnvVarUpdate} $0 "PATH" "A" "HKLM" "$PROGRAMFILES\http"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\uninstall http ${HTTP_VERSION}.exe"
  Delete "$INSTDIR\http.exe"
  Delete "$INSTDIR"
  ${un.EnvVarUpdate} $0 "PATH" "R" "HKLM" "$PROGRAMFILES\http"
SectionEnd
