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
  EnVar::SetHKLM
  EnVar::AddValue "PATH" "$PROGRAMFILES\http"
  Pop $0
  DetailPrint "Adding $PROGRAMFILES\http to %PATH%: $0"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\uninstall http ${HTTP_VERSION}.exe"
  Delete "$INSTDIR\http.exe"
  Delete "$INSTDIR"

  EnVar::SetHKLM
  EnVar::DeleteValue "PATH" "$PROGRAMFILES\http"
  Pop $0
  DetailPrint "deleting $PROGRAMFILES\http from %PATH%: $0"
SectionEnd
