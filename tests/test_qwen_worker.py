"""Boundary regression tests; no model download/GPU is needed for these."""
import importlib.util
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("worker", Path(__file__).parents[1] / "src-tauri/src/qwen_worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class TextBoundaries(unittest.TestCase):
    def test_long_russian_text_preserves_words_and_punctuation(self):
        text = "Сегодня мы проверяем озвучивание. Встреча начнётся в 14:30, 25 октября 2026 года!\n" * 80
        parts = worker.chunks(text)
        self.assertGreater(len(parts), 1)
        self.assertTrue(all(0 < len(part) <= 240 for part in parts))
        self.assertEqual(" ".join(parts).split(), text.split())

    def test_unbroken_token_is_bounded_without_loss(self):
        text = "я" * 500
        parts = worker.chunks(text)
        self.assertEqual("".join(parts), text)
        self.assertTrue(all(len(part) <= 240 for part in parts))

    def test_empty_and_oversized_requests_rejected(self):
        for text in ["  \n ", "я" * 20001]:
            with self.assertRaises(ValueError):
                worker.chunks(text)


class ResumableDownloads(unittest.TestCase):
    def test_ranges_append_after_existing_prefix_without_duplicate_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory) / "model.download"
            temporary.write_bytes(b"abc")
            def fake_curl(args, **kwargs):
                self.assertEqual(args[args.index("--range") + 1], "3-5")
                Path(args[args.index("--output") + 1]).write_bytes(b"def")
                return SimpleNamespace(returncode=0)
            with patch.object(worker.subprocess, "run", fake_curl), patch.object(worker.subprocess, "CREATE_NO_WINDOW", 0, create=True), patch.object(worker.time, "sleep"):
                worker.parallel_windows_download("https://example.invalid/model", temporary, 6, lambda value: None)
            self.assertEqual(temporary.read_bytes(), b"abcdef")
            self.assertFalse(temporary.with_suffix(".parts").exists())

    def test_completed_ranges_survive_a_failed_download_for_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory) / "model.download"
            temporary.write_bytes(b"abc")
            def fail(args, **kwargs):
                Path(args[args.index("--output") + 1]).write_bytes(b"d")
                return SimpleNamespace(returncode=28)
            with patch.object(worker.subprocess, "run", fail), patch.object(worker.subprocess, "CREATE_NO_WINDOW", 0, create=True), patch.object(worker.time, "sleep"):
                with self.assertRaises(RuntimeError):
                    worker.parallel_windows_download("https://example.invalid/model", temporary, 6, lambda value: None)
            self.assertEqual(temporary.read_bytes(), b"abc")
            self.assertTrue(temporary.with_suffix(".parts").is_dir())


if __name__ == "__main__":
    unittest.main()
