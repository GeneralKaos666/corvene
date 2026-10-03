; Corvene's Windows installer (Inno Setup 6), compiled by package.ps1:
;   iscc /DAppVersion=<version> /DArch=<x86_64|aarch64|i686> [/DBaseName=Corvene-Full]
;        /DStage=<folder> /DOut=<folder> corvene.iss
;
; Per user, like GitHub Desktop's: no administrator rights, installed into
; %LOCALAPPDATA%\Programs\Corvene.
;   corvene.exe          the app
;   bin\corvene.bat      the command line tool (its folder goes on the user's PATH)
;   unins000.exe         what tells Corvene it is installed (url_schemes.rs, updater.rs)
; The Start menu shortcut carries the AppUserModelID toasts are shown under,
; and the x-corvene / x-corvene-auth URL schemes point at the app.
; An update is this same installer run with /VERYSILENT once Corvene has
; exited (crates/corvene-platform/src/app_location.rs).

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
  #define BaseName "Corvene"
#endif
#ifndef Out
  #define Out "."
#endif

[Setup]
AppId={{6F1D3C0A-5B7E-4E0C-9E57-3C6B1B0D8A42}
AppName=Corvene
AppVersion={#AppVersion}
AppVerName=Corvene {#AppVersion}
AppPublisher=Wasi Master
AppPublisherURL=https://github.com/wasi-master/corvene
AppSupportURL=https://github.com/wasi-master/corvene/issues
DefaultDirName={localappdata}\Programs\Corvene
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
OutputDir={#Out}
OutputBaseFilename={#BaseName}-{#AppVersion}-{#Arch}-setup
SetupIconFile=corvene.ico
UninstallDisplayIcon={app}\corvene.exe
UninstallDisplayName=Corvene
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
; Corvene exits before its updater runs this; a first install has nothing to close
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
Source: "{#Stage}\corvene.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\bin\corvene.bat"; DestDir: "{app}\bin"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Corvene"; Filename: "{app}\corvene.exe"; AppUserModelID: "com.wasimaster.corvene"
Name: "{userdesktop}\Corvene"; Filename: "{app}\corvene.exe"; AppUserModelID: "com.wasimaster.corvene"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\x-corvene"; ValueType: string; ValueName: ""; ValueData: "URL:x-corvene"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\x-corvene"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\x-corvene\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\x-corvene-auth"; ValueType: string; ValueName: ""; ValueData: "URL:x-corvene-auth"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\x-corvene-auth"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\x-corvene-auth\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"" ""%1"""

[Run]
Filename: "{app}\corvene.exe"; Description: "{cm:LaunchProgram,Corvene}"; Flags: nowait postinstall skipifsilent

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
