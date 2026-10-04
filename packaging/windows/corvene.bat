@echo off
rem Corvene command line tool for Windows (GitHub Desktop's github.bat;
rem packaging/corvene.sh and packaging/linux/corvene.sh are its twins):
rem   corvene                            open the current directory
rem   corvene open [path]                open the provided path
rem   corvene clone [-b branch] <url>    clone the repository by url or
rem                                      owner/name (ex torvalds/linux),
rem                                      optionally checking out the branch
rem   corvene list [--json]              list the repositories Corvene has
rem                                      (flag 425-cli-list-repositories)
rem   corvene open [path] --changes|--history
rem                                      open the path on that tab
rem Installed as <app>\bin\corvene.bat next to <app>\corvene.exe; the
rem installer puts <app>\bin on the user's PATH. Corvene itself turns the
rem arguments after --cli into an x-corvene:// URL (crates/corvene/src/
rem cli_windows.rs) and hands it to the running Corvene or starts one.
setlocal
if "%~1"=="-h" goto usage
if "%~1"=="--help" goto usage
if /i "%~1"=="help" goto usage
if /i "%~1"=="list" goto list
if not defined CORVENE_BIN set "CORVENE_BIN=%~dp0..\corvene.exe"
rem detached from the terminal, like GitHub Desktop's CLI
start "" "%CORVENE_BIN%" --cli %*
exit /b 0

:list
rem written by Corvene whenever the list changes (the store is locked while
rem it runs)
set "CORVENE_DATA=%APPDATA%\Corvene"
if defined CORVENE_DATA_DIR set "CORVENE_DATA=%CORVENE_DATA_DIR%"
set "LIST_FILE=%CORVENE_DATA%\repositories.txt"
if /i "%~2"=="--json" set "LIST_FILE=%CORVENE_DATA%\repositories.json"
if not exist "%LIST_FILE%" (
  echo corvene: no repository list yet ^(start Corvene, with flag 425-cli-list-repositories on^) 1>&2
  exit /b 1
)
type "%LIST_FILE%"
exit /b 0

:usage
echo Corvene CLI usage:
echo   corvene                            Open the current directory
echo   corvene open [path]                Open the provided path
echo   corvene clone [-b branch] ^<url^>    Clone the repository by url or name/owner
echo                                      (ex torvalds/linux), optionally checking out
echo                                      the branch
echo   corvene list [--json]              List the repositories Corvene has
echo   corvene open [path] --changes      Open the path on the Changes (or --history)
echo                                      tab
exit /b 0
