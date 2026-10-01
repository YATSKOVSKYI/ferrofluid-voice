import importlib.util
from pathlib import Path
import unittest
import sys
sys.path.insert(0, str(Path(__file__).parents[1] / "src-tauri/src"))
from meetings_advanced import aligned_rows, overlap_regions, canonical_text

spec = importlib.util.spec_from_file_location("worker", Path(__file__).parents[1] / "src-tauri/src/meetings_worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class MeetingAlignmentTests(unittest.TestCase):
    def test_forced_alignment_splits_words_without_losing_source_text(self):
        original = [dict(id="1", start=0, end=3, text="Привет,   как дела?", speakerId=None, needsReview=True)]
        aligned = [dict(words=[dict(word="Привет,", start=0, end=1), dict(word="как", start=1, end=2), dict(word="дела?", start=2, end=3)])]
        result = aligned_rows(original, aligned, [dict(start=0, end=1, speakerId="a"), dict(start=1, end=3, speakerId="b")])
        self.assertEqual([s["speakerId"] for s in result], ["a", "b"])
        self.assertEqual(canonical_text(result), canonical_text(original))

    def test_missing_word_timing_keeps_all_text_for_review(self):
        source = [dict(id="1", start=0, end=3, text="Price 2014.", speakerId="a", needsReview=False)]
        aligned = [dict(words=[dict(word="Price", start=0, end=1), dict(word="2014.")])]
        result = aligned_rows(source, aligned, [dict(start=0, end=3, speakerId="a")])
        self.assertEqual(result[0]["text"], source[0]["text"])
        self.assertIsNone(result[0]["speakerId"])
        self.assertTrue(result[0]["needsReview"])

    def test_overlap_detection_and_locked_text(self):
        turns = [dict(start=0, end=2, speakerId="a"), dict(start=1, end=3, speakerId="b")]
        self.assertEqual(overlap_regions(turns), [dict(start=1, end=2)])
        source = [dict(id="1", start=0, end=3, text="Manual", speakerId="a", needsReview=False, speakerLocked=True)]
        self.assertEqual(aligned_rows(source, [dict(words=[])], turns), source)

    def test_calibration_abstains_for_close_or_weak_matches(self):
        self.assertIsNone(worker.classify_similarity([0.72, 0.65], ["a", "b"]))
        self.assertIsNone(worker.classify_similarity([0.48, 0.2], ["a", "b"]))
        self.assertEqual(worker.classify_similarity([0.8, 0.3], ["a", "b"]), "a")

    def test_calibration_preserves_text_and_manual_assignments(self):
        rows = [dict(id="1", start=0, end=4, text="Не менять текст", speakerId="old", needsReview=False),
                dict(id="2", start=0, end=4, text="Ручная правка", speakerId="old", needsReview=False, speakerLocked=True)]
        result = worker.relabel_segments(rows, [dict(start=0, end=2, speakerId="a"), dict(start=2, end=4, speakerId="b")])
        self.assertIsNone(result[0]["speakerId"])
        self.assertTrue(result[0]["needsReview"])
        self.assertEqual(result[0]["text"], rows[0]["text"])
        self.assertEqual(result[1], rows[1])

    def test_overlapping_speakers_require_review(self):
        turns = [dict(start=0, end=2, speakerId="a"), dict(start=1, end=2, speakerId="b")]
        self.assertEqual(worker.assign_speaker(0, 2, turns), ("a", True))

    def test_uncovered_speech_is_not_guessed(self):
        self.assertEqual(worker.assign_speaker(10, 12, [dict(start=0, end=1, speakerId="a")]), (None, True))

    def test_timed_words_split_at_speaker_change_and_add_chunk_offset(self):
        turns = [dict(start=120, end=121, speakerId="a"), dict(start=121, end=122, speakerId="b")]
        result = {"transcription": [{"text": " Hello there", "offsets": {"from": 0, "to": 2000}, "tokens": [
            {"text": " Hello", "offsets": {"from": 0, "to": 1000}},
            {"text": " there", "offsets": {"from": 1000, "to": 2000}}]}]}
        rows = worker.align_segments(result, 120, turns)
        self.assertEqual([r["speakerId"] for r in rows], ["a", "b"])
        self.assertEqual([r["text"] for r in rows], ["Hello", "there"])
        self.assertEqual(rows[-1]["end"], 122)

    def test_untimed_tokens_do_not_drop_words(self):
        result = {"transcription": [{"text": " Hello world", "offsets": {"from": 0, "to": 2000}, "tokens": [
            {"text": " Hello", "offsets": {"from": 0, "to": 1000}},
            {"text": " world", "offsets": {"from": -1, "to": -1}}]}]}
        rows = worker.align_segments(result, 0, [dict(start=0, end=2, speakerId="a")])
        self.assertEqual(rows[0]["text"], "Hello world")

    def test_nearby_speech_remains_provisional(self):
        self.assertEqual(worker.assign_speaker(1.1, 1.2, [dict(start=0, end=1, speakerId="a")]), ("a", True))

    def test_grouping_keeps_review_flag(self):
        result = {"transcription": [
            {"text": " Hello", "offsets": {"from": 0, "to": 1000}},
            {"text": " world", "offsets": {"from": 1100, "to": 1200}}]}
        rows = worker.align_segments(result, 0, [dict(start=0, end=1, speakerId="a")])
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["text"], "Hello world")
        self.assertTrue(rows[0]["needsReview"])


if __name__ == "__main__":
    unittest.main()
