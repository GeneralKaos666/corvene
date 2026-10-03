; The MinGit the installer offers when no git is on the PC (corvene.iss,
; the `mingit` task), written by mingit.py: do not edit by hand.
#define MinGitVersion "2.56.0"
#define MinGitTag "v2.56.0.windows.1"
#define MinGitUrl "https://github.com/git-for-windows/git/releases/download/v2.56.0.windows.1/"
#if Arch == "x86_64"
  #define MinGitFile "MinGit-2.56.0-64-bit.zip"
  #define MinGitExtractedSize 95772593
  #define MinGitSha256 "064b440ff870ed5198527e8f3a92cdf5bd2fd0fedf5e718af95e3fdaddeff718"
#elif Arch == "i686"
  #define MinGitFile "MinGit-2.56.0-32-bit.zip"
  #define MinGitExtractedSize 94186091
  #define MinGitSha256 "9f8266486c8818b91cbb6b719e35972a406f7560e86b82fbda2f3dcb7c069f12"
#elif Arch == "aarch64"
  #define MinGitFile "MinGit-2.56.0-arm64.zip"
  #define MinGitExtractedSize 90115183
  #define MinGitSha256 "cb3b0f2d486ea52673227151a5baf5bc13861ff80e74e94e46d614d1bfcd5c06"
#else
  #error no MinGit for this architecture
#endif
