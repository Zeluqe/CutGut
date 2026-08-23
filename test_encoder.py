import unittest
import encoder

class TestEncoderCalculations(unittest.TestCase):
    def test_target_bytes_dual_limit(self):
        # Dla 20 MB: min(0.995 * 20 * 1024^2, 0.98 * 20 * 1000^2) -> 19,600,000 B
        bytes_20 = encoder.calculate_target_bytes(20.0)
        self.assertEqual(bytes_20, 19600000)
        self.assertLessEqual(bytes_20, 20000000)
        self.assertLessEqual(bytes_20, 20 * 1024 * 1024)

        # Dla 10 MB: 9,800,000 B
        bytes_10 = encoder.calculate_target_bytes(10.0)
        self.assertEqual(bytes_10, 9800000)

        # Dla 50 MB: 49,000,000 B
        bytes_50 = encoder.calculate_target_bytes(50.0)
        self.assertEqual(bytes_50, 49000000)

        # Dla 500 MB: 490,000,000 B
        bytes_500 = encoder.calculate_target_bytes(500.0)
        self.assertEqual(bytes_500, 490000000)

    def test_plan_calculation_high_bitrate(self):
        # 10 sekund na 20 MB -> wysoki bitrate, brak downscalingu
        v_info = encoder.VideoInfo(duration=60.0, width=1920, height=1080, fps=60.0, bitrate=5000000, codec='h264', audio_codec='aac')
        plan = encoder.calculate_plan(v_info, start_s=0.0, end_s=10.0, target_mb=20.0)
        
        self.assertEqual(plan['duration_s'], 10.0)
        self.assertIsNone(plan['filter_str'])
        self.assertGreater(plan['video_kbps'], 1000)

    def test_plan_calculation_720p_downscale(self):
        # Długi klip: 200 sekund na 20 MB -> bitrate ~700 kbps (<900k), downscale do 720p
        v_info = encoder.VideoInfo(duration=300.0, width=1920, height=1080, fps=60.0, bitrate=5000000, codec='h264', audio_codec='aac')
        plan = encoder.calculate_plan(v_info, start_s=0.0, end_s=200.0, target_mb=20.0)
        
        self.assertIsNotNone(plan['filter_str'])
        self.assertIn('scale=-2:720', plan['filter_str'])
        self.assertIn('fps=30', plan['filter_str'])

    def test_plan_calculation_480p_downscale(self):
        # Bardzo długi klip: 400 sekund na 20 MB -> bitrate < 450 kbps, downscale do 480p i 30fps
        v_info = encoder.VideoInfo(duration=600.0, width=1920, height=1080, fps=60.0, bitrate=5000000, codec='h264', audio_codec='aac')
        plan = encoder.calculate_plan(v_info, start_s=0.0, end_s=400.0, target_mb=20.0)
        
        self.assertIsNotNone(plan['filter_str'])
        self.assertIn('scale=-2:480', plan['filter_str'])
        self.assertIn('fps=30', plan['filter_str'])

    def test_cancellation_token(self):
        token = encoder.CancellationToken()
        self.assertFalse(token.cancelled)
        token.cancel()
        self.assertTrue(token.cancelled)

    def test_best_encoder_selection(self):
        best = encoder.get_best_available_encoder()
        self.assertIn(best, ['NVENC_HQ', 'AMF_HQ', 'CPU_BALANCED'])
    def test_encode_job_creation(self):
        job = encoder.EncodeJob(
            job_id='1',
            input_path='test.mp4',
            output_path='out.mp4',
            start_s=0.0,
            end_s=10.0,
            target_mb=20.0,
            preset_mode='NVENC_HQ'
        )
        self.assertEqual(job.status, 'pending')
        self.assertEqual(job.progress_pct, 0.0)

    def test_can_stream_copy_estimation(self):
        # Wideo 60s o bitrate 1 Mbps (125 KB/s) -> wycinek 10s to ~1.25 MB, mieści się w limicie 20 MB
        v_info = encoder.VideoInfo(
            duration=60.0, width=1920, height=1080, fps=60.0,
            bitrate=1000000, codec='h264', audio_codec='aac'
        )
        # Przy braku istniejacego pliku na dysku zwraca False bezpiecznie
        self.assertFalse(encoder.can_stream_copy(v_info, 'non_existent_file.mp4', 0.0, 10.0, 20.0))

    def test_quality_assessment_bppf(self):
        # 1. 1080p60 przy ~2469 kbps -> bppf ~0.0198 -> rating "ok" ("OK do wysłania")
        q_ok = encoder.assess_quality(video_kbps=2469, width=1920, height=1080, fps=60.0, dur_s=60.0, lang='pl')
        self.assertEqual(q_ok.rating, 'ok')
        self.assertIn('OK do wysłania', q_ok.label)
        self.assertGreater(len(q_ok.tip), 0)

        # 2. 720p30 przy ~2500 kbps -> bppf ~0.09 -> rating "great" ("Świetna jakość")
        q_great = encoder.assess_quality(video_kbps=2500, width=1280, height=720, fps=30.0, dur_s=15.0, lang='pl')
        self.assertEqual(q_great.rating, 'great')
        self.assertIn('Świetna', q_great.label)

        # 3. 1080p60 przy bardzo niskim bitrate ~500 kbps -> rating "very_low"
        q_low = encoder.assess_quality(video_kbps=500, width=1920, height=1080, fps=60.0, dur_s=300.0, lang='pl')
        self.assertIn(q_low.rating, ['low', 'very_low'])

    def test_cleanup_policy_safety(self):
        # 1. NEVER zwraca False i nie usuwa niczego
        ok, msg = encoder.cleanup_source_file('some_file.mp4', 'out.mp4', encoder.SourceCleanupPolicy.NEVER)
        self.assertFalse(ok)

        # 2. Ta sama ścieżka wejścia i wyjścia jest bezpiecznie blokowana
        ok, msg = encoder.cleanup_source_file('video.mp4', 'video.mp4', encoder.SourceCleanupPolicy.TRASH)
        self.assertFalse(ok)
        self.assertIn('zablokowana', msg)

if __name__ == '__main__':
    unittest.main()
