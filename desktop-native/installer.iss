; Per-user installer for the native beta (no admin), built by release-native.yml:
;   iscc /DAppVersion=0.1.5 /DSourceExe=path\to\desktop-native.exe /DOutputDir=release-assets installer.iss
; Uninstalling also removes the autostart entry the app writes, and, when the user agrees, its settings.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\native-engine\target\release\desktop-native.exe"
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
; Not the Tauri app's id, so both install side by side.
AppId={{6F0D7E52-3C1B-4B7A-9E35-5A2C8F1D4E90}
AppName=Screenshare nativo
AppVersion={#AppVersion}
AppPublisher=Rafael Rain
AppPublisherURL=https://github.com/RafaelrainBR/screenshare-kotlin-js
DefaultDirName={localappdata}\Programs\Screenshare-native
DefaultGroupName=Screenshare nativo
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=Screenshare-native_{#AppVersion}_x64-setup
SetupIconFile=..\desktop-tauri\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\icon.ico
UninstallDisplayName=Screenshare nativo
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; A running app holds the exe. It hides to the tray when asked to close, so the Restart Manager must force it.
CloseApplications=force
RestartApplications=no

[Languages]
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "Criar atalho na área de trabalho"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "Screenshare-native.exe"; Flags: ignoreversion
Source: "..\desktop-tauri\src-tauri\icons\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Screenshare nativo"; Filename: "{app}\Screenshare-native.exe"; IconFilename: "{app}\icon.ico"
Name: "{autodesktop}\Screenshare nativo"; Filename: "{app}\Screenshare-native.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\Screenshare-native.exe"; Description: "Abrir o Screenshare nativo"; Flags: nowait postinstall skipifsilent
; The app's updater (src/updater.rs) runs this setup with /VERYSILENT and quits: open it again.
Filename: "{app}\Screenshare-native.exe"; Flags: nowait; Check: WizardSilent

[Code]
const
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  RunValue = 'Screenshare-native';

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Code: Integer;
begin
  // The app hides to the tray on close requests; end it so its exe can be removed.
  if CurUninstallStep = usUninstall then
    Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM Screenshare-native.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  if CurUninstallStep = usPostUninstall then
  begin
    // startup.rs writes this value for "Iniciar com o sistema"; left behind it would start a missing exe.
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, RunValue);
    // Settings: last room, transmission profile, sound. Kept unless the user says so (and in silent mode).
    if DirExists(ExpandConstant('{userappdata}\Screenshare-native')) and
       (SuppressibleMsgBox('Remover também as configurações do Screenshare nativo (última sala, transmissão e som)?',
          mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) = IDYES) then
      DelTree(ExpandConstant('{userappdata}\Screenshare-native'), True, True, True);
  end;
end;
