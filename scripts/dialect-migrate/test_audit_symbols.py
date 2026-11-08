import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name('audit_symbols.py').resolve()


class AuditSymbolsTest(unittest.TestCase):
    def test_detects_removed_symbols_and_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'src').mkdir()
            (root / 'src/a.rs').write_text('fn keep() {}\nfn missing() {}\n')
            (root / 'src/b.rs').write_text('struct Gone;\n')
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            subprocess.run(['git', '-C', str(root), 'add', 'src'], check=True)
            subprocess.run(['git', '-C', str(root), '-c', 'user.name=test', '-c',
                            'user.email=test@example.com', 'commit', '-qm', 'baseline'], check=True)
            env = {**os.environ, 'BASE': 'HEAD'}
            def audit():
                return subprocess.run(['python3', str(SCRIPT)], cwd=root, env=env,
                                      capture_output=True, text=True)
            self.assertEqual(audit().returncode, 0)
            (root / 'src/a.rs').write_text('fn keep() {}\n')
            (root / 'src/b.rs').unlink()
            result = audit()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('missing', result.stdout)
            self.assertIn('src/b.rs', result.stdout)


if __name__ == '__main__':
    unittest.main()
