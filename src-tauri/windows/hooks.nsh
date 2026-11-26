; Menú contextual "Organizar con Wonderlust" para carpetas (usuario actual).
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\Directory\shell\WonderlustOrganizar" "MUIVerb" "Organizar con Wonderlust"
  WriteRegStr HKCU "Software\Classes\Directory\shell\WonderlustOrganizar" "Icon" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr HKCU "Software\Classes\Directory\shell\WonderlustOrganizar\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\Directory\shell\WonderlustOrganizar"
!macroend
