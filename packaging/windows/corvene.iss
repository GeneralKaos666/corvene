; Corvene's Windows installer (Inno Setup 6.7 or 7), compiled by package.ps1:
;   iscc /DAppVersion=<version> /DArch=<x86_64|aarch64|i686> [/DBaseName=Corvene-Full]
;        /DStage=<folder> /DOut=<folder> [/DSign "/Ssigntool=<command> $f"] corvene.iss
;
; Per user, like GitHub Desktop's: no administrator rights, installed into
; %LOCALAPPDATA%\Programs\Corvene.
;   corvene.exe          the app
;   bin\corvene.bat      the command line tool (its folder goes on the user's PATH)
;   git\                 MinGit, only when the PC had no git (mingit.iss, the
;                        `mingit` task; corvene_git::detect looks there)
;   unins000.exe         what tells Corvene it is installed (corvene_platform::windows)
; The Start menu shortcut carries the AppUserModelID toasts are shown under
; and the toast activator CLSID whose COM server is corvene.exe
; (corvene_platform::notifications), the x-corvene / x-corvene-auth URL
; schemes point at the app, and `corvene` resolves from Win+R (App Paths).
; Optional: "Open in Corvene" in Explorer's folder menus (`explorermenu`).
; A running Corvene is closed by the Restart Manager before its files are
; replaced and started again afterwards (RegisterApplicationRestart in
; corvene_platform::windows); the uninstaller refuses while the
; `CorveneRunning` mutex exists.
; An update is this same installer run with /VERYSILENT once Corvene has
; exited (crates/corvene-platform/src/app_location.rs, which also passes
; /NORESTARTAPPLICATIONS and /LOG).
; With /DSign, Inno signs the installer and the uninstaller with the
; `signtool` Sign Tool package.ps1 defines on the command line (package.ps1
; signs corvene.exe itself before staging it).

#if Ver < EncodeVer(6, 7, 0, 0)
  #error Inno Setup 6.7 or later is needed (ArchiveExtraction, png wizard images, WizardStyle dynamic)
#endif
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
; the version resource wants digits only: "0.2.0-beta.1" -> "0.2.0"
#define NumVersion Copy(AppVersion, 1, Pos("-", AppVersion + "-") - 1)
#include "mingit.iss"

; a [Languages] line for one of Inno Setup's translations, when this
; installation of Inno Setup has it (the set grew over the versions)
#define LangLine(str Name, str File) FileExists(CompilerPath + "\Languages\" + File) ? 'Name: "' + Name + '"; MessagesFile: "compiler:Languages\' + File + '"' : ""

[Setup]
AppId={{6F1D3C0A-5B7E-4E0C-9E57-3C6B1B0D8A42}
AppName=Corvene
AppVersion={#AppVersion}
AppVerName=Corvene {#AppVersion}
AppPublisher=Wasi Master
AppPublisherURL=https://github.com/wasi-master/corvene
AppSupportURL=https://github.com/wasi-master/corvene/issues
AppUpdatesURL=https://github.com/wasi-master/corvene/releases
AppCopyright=Copyright (c) 2026 Wasi Master
VersionInfoVersion={#NumVersion}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoCompany=Wasi Master
VersionInfoDescription=Corvene Setup
VersionInfoProductName=Corvene
VersionInfoCopyright=Copyright (c) 2026 Wasi Master
DefaultDirName={localappdata}\Programs\Corvene
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
; Windows 10 1809: what GPUI's DirectX backend and the WinRT toasts need
MinVersion=10.0.17763
OutputDir={#Out}
OutputBaseFilename={#BaseName}-{#AppVersion}-{#Arch}-setup
SetupIconFile=corvene.ico
; one Setup at a time (the updater's silent run and a hand-started one)
SetupMutex=CorveneSetup
UninstallDisplayIcon={app}\corvene.exe
UninstallDisplayName=Corvene
; Apps & Features' uninstall writes a log to %TEMP% (Setup's goes where
; the updater's /LOG says)
UninstallLogging=yes
Compression=lzma2
SolidCompression=yes
; follows the system's light or dark mode, like Corvene's theme
WizardStyle=modern dynamic
; the icon on transparent, one file per DPI scale (Setup picks the closest)
WizardImageFile=wizard\large-100.png,wizard\large-125.png,wizard\large-150.png,wizard\large-175.png,wizard\large-200.png,wizard\large-250.png
WizardSmallImageFile=wizard\small-100.png,wizard\small-125.png,wizard\small-150.png,wizard\small-175.png,wizard\small-200.png,wizard\small-250.png
; the wizard speaks the system's language (GitHub Desktop's installer has
; no interface at all); Corvene itself is English
ShowLanguageDialog=no
ChangesEnvironment=yes
; Restart Manager: a running Corvene is closed before its files are
; replaced and, having called RegisterApplicationRestart, started again
; afterwards. The updater runs Setup once Corvene has exited and restarts
; it itself (/NORESTARTAPPLICATIONS).
CloseApplications=yes
CloseApplicationsFilter=corvene.exe
RestartApplications=yes
; the MinGit zip (basic only reads .7z)
ArchiveExtraction=enhanced/nopassword
#ifdef Sign
SignTool=signtool
SignedUninstaller=yes
SignToolRetryCount=3
#endif
; the 32-bit build installs anywhere
#if Arch == "aarch64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#elif Arch == "x86_64"
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
#emit LangLine("arabic", "Arabic.isl")
#emit LangLine("armenian", "Armenian.isl")
#emit LangLine("brazilianportuguese", "BrazilianPortuguese.isl")
#emit LangLine("bulgarian", "Bulgarian.isl")
#emit LangLine("catalan", "Catalan.isl")
#emit LangLine("chinesesimplified", "ChineseSimplified.isl")
#emit LangLine("chinesetraditional", "ChineseTraditional.isl")
#emit LangLine("corsican", "Corsican.isl")
#emit LangLine("czech", "Czech.isl")
#emit LangLine("danish", "Danish.isl")
#emit LangLine("dutch", "Dutch.isl")
#emit LangLine("finnish", "Finnish.isl")
#emit LangLine("french", "French.isl")
#emit LangLine("german", "German.isl")
#emit LangLine("hebrew", "Hebrew.isl")
#emit LangLine("hungarian", "Hungarian.isl")
#emit LangLine("italian", "Italian.isl")
#emit LangLine("japanese", "Japanese.isl")
#emit LangLine("korean", "Korean.isl")
#emit LangLine("lithuanian", "Lithuanian.isl")
#emit LangLine("norwegian", "Norwegian.isl")
#emit LangLine("polish", "Polish.isl")
#emit LangLine("portuguese", "Portuguese.isl")
#emit LangLine("russian", "Russian.isl")
#emit LangLine("slovak", "Slovak.isl")
#emit LangLine("slovenian", "Slovenian.isl")
#emit LangLine("spanish", "Spanish.isl")
#emit LangLine("swedish", "Swedish.isl")
#emit LangLine("tamil", "Tamil.isl")
#emit LangLine("thai", "Thai.isl")
#emit LangLine("turkish", "Turkish.isl")
#emit LangLine("ukrainian", "Ukrainian.isl")

; Corvene's own wizard text (English for every language, like the app)
[CustomMessages]
IntegrationGroup=Windows integration:
ExplorerMenuTask=Add "Open in Corvene" to the menu of folders in Explorer
OpenInCorvene=Open in Corvene
GitGroup=Git:
MinGitTask=Download and install Git %1 for Corvene (no Git was found on this PC, about 40 MB)
CloseBeforeUninstall=Corvene is running. Close it, then click Retry.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "explorermenu"; Description: "{cm:ExplorerMenuTask}"; GroupDescription: "{cm:IntegrationGroup}"; Flags: unchecked
; only offered when neither the PATH nor Git for Windows' folders have a git
Name: "mingit"; Description: "{cm:MinGitTask,{#MinGitVersion}}"; GroupDescription: "{cm:GitGroup}"; Check: GitMissing

[Files]
Source: "{#Stage}\corvene.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\bin\corvene.bat"; DestDir: "{app}\bin"; Flags: ignoreversion
; MinGit from Git for Windows' release, downloaded during Setup, checked
; against the pinned sha256 and unpacked into {app}\git (cmd\git.exe)
Source: "{#MinGitUrl}{#MinGitFile}"; DestDir: "{app}\git"; DestName: "{#MinGitFile}"; ExternalSize: {#MinGitExtractedSize}; Hash: "{#MinGitSha256}"; Flags: external download extractarchive recursesubdirs createallsubdirs ignoreversion; Tasks: mingit

[Icons]
Name: "{userprograms}\Corvene"; Filename: "{app}\corvene.exe"; AppUserModelID: "com.wasimaster.corvene"; AppUserModelToastActivatorCLSID: "3405A3D0-042A-4FAB-A267-FD8A6E9608CF"
Name: "{userdesktop}\Corvene"; Filename: "{app}\corvene.exe"; AppUserModelID: "com.wasimaster.corvene"; AppUserModelToastActivatorCLSID: "3405A3D0-042A-4FAB-A267-FD8A6E9608CF"; Tasks: desktopicon

[Registry]
; the URL schemes (corvene_platform::url_schemes repairs them at launch)
Root: HKCU; Subkey: "Software\Classes\x-corvene"; ValueType: string; ValueName: ""; ValueData: "URL:x-corvene"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\x-corvene"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\x-corvene\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\x-corvene-auth"; ValueType: string; ValueName: ""; ValueData: "URL:x-corvene-auth"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\x-corvene-auth"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\x-corvene-auth\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"" ""%1"""
; the toast activator's COM server (corvene_platform::windows::TOAST_ACTIVATOR_CLSID)
Root: HKCU; Subkey: "Software\Classes\CLSID\{{3405A3D0-042A-4FAB-A267-FD8A6E9608CF}"; ValueType: string; ValueName: ""; ValueData: "Corvene notification activator"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\CLSID\{{3405A3D0-042A-4FAB-A267-FD8A6E9608CF}\LocalServer32"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"""
; App Paths: `corvene` from Win+R and ShellExecute, `corvene.bat` from its folder
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\corvene.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\corvene.exe"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\corvene.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}\bin"
; Explorer: "Open in Corvene" on a folder and inside one (%V is the folder)
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Corvene"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenInCorvene}"; Flags: uninsdeletekey; Tasks: explorermenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Corvene"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\corvene.exe"""; Tasks: explorermenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Corvene\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"" --cli open ""%V"""; Tasks: explorermenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Corvene"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenInCorvene}"; Flags: uninsdeletekey; Tasks: explorermenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Corvene"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\corvene.exe"""; Tasks: explorermenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Corvene\command"; ValueType: string; ValueName: ""; ValueData: """{app}\corvene.exe"" --cli open ""%V"""; Tasks: explorermenu

[UninstallDelete]
; the MinGit's files (extracted, so not in the uninstall log) and the
; cache (update downloads); settings and the store stay, like GitHub Desktop's
Type: filesandordirs; Name: "{app}\git"
Type: filesandordirs; Name: "{localappdata}\Corvene\Cache"

[Run]
Filename: "{app}\corvene.exe"; Description: "{cm:LaunchProgram,Corvene}"; Flags: nowait postinstall skipifsilent

[Code]
const
  EnvironmentKey = 'Environment';
  { what a running Corvene holds (corvene_platform::windows::hold_running_mutex) }
  RunningMutex = 'CorveneRunning';

{ Whether no git.exe is on the PATH or where Git for Windows installs
  (corvene_git::detect::windows_candidates); the `mingit` task then shows. }
function GitMissing: Boolean;
begin
  Result := (FileSearch('git.exe', GetEnv('PATH')) = '')
    and not FileExists(GetEnv('ProgramFiles') + '\Git\cmd\git.exe')
    and not FileExists(GetEnv('ProgramW6432') + '\Git\cmd\git.exe')
    and not FileExists(GetEnv('ProgramFiles(x86)') + '\Git\cmd\git.exe')
    and not FileExists(ExpandConstant('{localappdata}\Programs\Git\cmd\git.exe'));
end;

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

{ A running Corvene has its files in use: ask to close it first (Setup
  itself has the Restart Manager for that; the uninstaller has not). A
  silent uninstall goes ahead and leaves what is in use. }
function InitializeUninstall(): Boolean;
begin
  Result := True;
  if UninstallSilent then
    Exit;
  while CheckForMutexes(RunningMutex) do
    if MsgBox(CustomMessage('CloseBeforeUninstall'), mbError, MB_RETRYCANCEL) = IDCANCEL then
    begin
      Result := False;
      Exit;
    end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    SetBinOnPath(False);
end;
