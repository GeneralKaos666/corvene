; Corvane's Windows installer (Inno Setup 6), compiled by package.ps1:
;   iscc /DAppVersion=<version> /DArch=<x86_64|aarch64|i686> [/DBaseName=Corvane-Full]
;        /DStage=<folder> /DOut=<folder> corvane.iss
;
; Per user, like GitHub Desktop's: no administrator rights, installed into
; %LOCALAPPDATA%\Programs\Corvane.
;   corvane.exe          the app
;   bin\corvane.bat      the command line tool (its folder goes on the user's PATH)
;   unins000.exe         what tells Corvane it is installed (url_schemes.rs, updater.rs)
; The Start menu shortcut carries the AppUserModelID toasts are shown under,
; and the x-corvane / x-corvane-auth URL schemes point at the app.
; An update is this same installer run with /VERYSILENT once Corvane has
; exited (crates/corvane-platform/src/app_location.rs).

#ifndef AppVersion
  #error AppVersion is not defined
#endif
#ifndef Arch
  #define Arch "x86_64"
#endif
#ifndef Stage
  #error Stage is not defined
#endif
#ifndef BaseName
  #define BaseName "Corvane"
#endif
#ifndef Out
  #define Out "."
#endif

[Setup]
AppId={{6F1D3C0A-5B7E-4E0C-9E57-3C6B1B0D8A42}
AppName=Corvane
AppVersion={#AppVersion}
AppVerName=Corvane {#AppVersion}
AppPublisher=Wasi Master
AppPublisherURL=https://github.com/wasi-master/corvane
AppSupportURL=https://github.com/wasi-master/corvane/issues
DefaultDirName={localappdata}\Programs\Corvane
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
OutputDir={#Out}
OutputBaseFilename={#BaseName}-{#AppVersion}-{#Arch}-setup
SetupIconFile=corvane.ico
UninstallDisplayIcon={app}\corvane.exe
UninstallDisplayName=Corvane
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
; Corvane exits before its updater runs this; a first install has nothing to close
CloseApplications=no
; the 32-bit build installs anywhere
#if Arch == "aarch64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#elif Arch == "x86_64"
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#Stage}\corvane.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\bin\corvane.bat"; DestDir: "{app}\bin"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Corvane"; Filename: "{app}\corvane.exe"; AppUserModelID: "com.wasimaster.corvane"
Name: "{userdesktop}\Corvane"; Filename: "{app}\corvane.exe"; AppUserModelID: "com.wasimaster.corvane"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\x-corvane"; ValueType: string; ValueName: ""; ValueData: "URL:x-corvane"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\x-corvane"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\x-corvane\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvane.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\x-corvane-auth"; ValueType: string; ValueName: ""; ValueData: "URL:x-corvane-auth"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\x-corvane-auth"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\x-corvane-auth\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvane.exe"" ""%1"""

[Run]
Filename: "{app}\corvane.exe"; Description: "{cm:LaunchProgram,Corvane}"; Flags: nowait postinstall skipifsilent

[Code]
const
  EnvironmentKey = 'Environment';

{ The folders on the user's PATH, without the one given. }
function PathWithout(const Path, Folder: string): string;
var
  Rest, Item: string;
  Separator: Integer;
begin
  Result := '';
  Rest := Path;
  while Rest <> '' do
  begin
    Separator := Pos(';', Rest);
    if Separator = 0 then
    begin
      Item := Rest;
      Rest := '';
    end
    else
    begin
      Item := Copy(Rest, 1, Separator - 1);
      Rest := Copy(Rest, Separator + 1, Length(Rest));
    end;
    if (Item <> '') and (CompareText(Item, Folder) <> 0) then
    begin
      if Result <> '' then
        Result := Result + ';';
      Result := Result + Item;
    end;
  end;
end;

procedure SetBinOnPath(const Add: Boolean);
var
  Path, Folder: string;
begin
  Folder := ExpandConstant('{app}\bin');
  if not RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path) then
    Path := '';
  Path := PathWithout(Path, Folder);
  if Add then
  begin
    if Path <> '' then
      Path := Path + ';';
    Path := Path + Folder;
  end;
  RegWriteExpandStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', Path);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    SetBinOnPath(True);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    SetBinOnPath(False);
end;
