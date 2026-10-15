/* A console PE32: emit the supplied nonce and return a distinctive exit code.
 * Win32-only entrypoint also permits a CRT-free clang/lld build on maintainer hosts.
 */
#include <windows.h>

void mainCRTStartup(void) {
  char value[2];
  if (GetEnvironmentVariableA("APPDIR", value, sizeof(value)) ||
      GetEnvironmentVariableA("PYTHONHOME", value, sizeof(value)) ||
      GetEnvironmentVariableA("PYTHONPATH", value, sizeof(value)) ||
      GetEnvironmentVariableA("LD_LIBRARY_PATH", value, sizeof(value))) ExitProcess(3);
  char *command = GetCommandLineA();
  int quoted = *command == '"';
  if (quoted) command++;
  while (*command && (quoted ? *command != '"' : *command != ' ')) command++;
  if (quoted && *command) command++;
  while (*command == ' ') command++;
  /* Output is deliberately a fixed relative filename inside the disposable CWD. */
  DWORD size = 0, written = 0;
  while (command[size]) size++;
  HANDLE file = CreateFileA("nonce.txt", GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL);
  if (file == INVALID_HANDLE_VALUE) ExitProcess(1);
  BOOL ok = WriteFile(file, command, size, &written, NULL);
  if (!CloseHandle(file) || !ok || written != size) ExitProcess(2);
  ExitProcess(42);
}
