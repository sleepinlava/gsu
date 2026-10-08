import os
import sys
from pathlib import Path


def main():
    name = 'gsu.exe' if os.name == 'nt' else 'gsu'
    binary = Path(__file__).resolve().parent / 'bin' / name
    if not binary.is_file():
        sys.stderr.write(
            f'gsu: no bundled binary for this platform (expected {binary})\n'
        )
        raise SystemExit(2)
    # pip drops zip permission bits for package files, so restore the exec bit
    if not os.access(binary, os.X_OK):
        try:
            binary.chmod(0o755)
        except OSError as error:
            sys.stderr.write(f'gsu: {binary} is not executable: {error}\n')
            raise SystemExit(2)
    os.execv(str(binary), [str(binary), *sys.argv[1:]])


if __name__ == '__main__':
    main()
