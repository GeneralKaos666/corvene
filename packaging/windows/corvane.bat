@echo off
rem Corvane command line tool for Windows (GitHub Desktop's github.bat;
rem packaging/corvane.sh and packaging/linux/corvane.sh are its twins):
rem   corvane                            open the current directory
rem   corvane open [path]                open the provided path
rem   corvane clone [-b branch] <url>    clone the repository by url or
rem                                      owner/name (ex torvalds/linux),
rem                                      optionally checking out the branch
rem Installed as <app>\bin\corvane.bat next to <app>\corvane.exe; the
rem installer puts <app>\bin on the user's PATH. Corvane itself turns the
rem arguments after --cli into an x-corvane:// URL (crates/corvane/src/
rem cli_windows.rs) and hands it to the running Corvane or starts one.
setlocal
if "%~1"=="-h" goto usage
if "%~1"=="--help" goto usage
if /i "%~1"=="help" goto usage
if not defined CORVANE_BIN set "CORVANE_BIN=%~dp0..\corvane.exe"
rem detached from the terminal, like GitHub Desktop's CLI
start "" "%CORVANE_BIN%" --cli %*
exit /b 0

:usage
echo Corvane CLI usage:
echo   corvane                            Open the current directory
echo   corvane open [path]                Open the provided path
echo   corvane clone [-b branch] ^<url^>    Clone the repository by url or name/owner
echo                                      (ex torvalds/linux), optionally checking out
echo                                      the branch
exit /b 0
