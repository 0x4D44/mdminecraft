#!/usr/bin/env python3
import argparse
import json
import pathlib
import subprocess
import sys


def wslpath_win(path: pathlib.Path) -> str:
    result = subprocess.check_output(["wslpath", "-w", str(path)], text=True)
    return result.strip()


def build_commentary_txt(run_dir: pathlib.Path) -> pathlib.Path:
    jsonl_path = run_dir / "commentary.jsonl"
    txt_path = run_dir / "commentary.txt"
    if not jsonl_path.exists():
        print(f"commentary log not found: {jsonl_path}")
        return txt_path
    lines = []
    with jsonl_path.open("r", encoding="utf-8") as handle:
        for raw in handle:
            raw = raw.strip()
            if not raw:
                continue
            try:
                payload = json.loads(raw)
            except json.JSONDecodeError:
                continue
            text = payload.get("text")
            if text:
                lines.append(text)
    txt_path.write_text("\n".join(lines), encoding="utf-8")
    return txt_path


def synthesize_tts(commentary_txt: pathlib.Path, wav_path: pathlib.Path, voice: str) -> None:
    win_txt = wslpath_win(commentary_txt)
    win_wav = wslpath_win(wav_path)
    voice = voice.replace("'", "''")
    script = (
        "Add-Type -AssemblyName System.Speech; "
        "$speak = New-Object System.Speech.Synthesis.SpeechSynthesizer; "
        f"$speak.SelectVoice('{voice}'); "
        f"$speak.SetOutputToWaveFile('{win_wav}'); "
        f"$speak.Speak((Get-Content -Raw '{win_txt}'));"
    )
    subprocess.run(["powershell.exe", "-Command", script], check=True)


def build_video(frames_dir: pathlib.Path, fps: int, output_path: pathlib.Path) -> None:
    frame_pattern = frames_dir / "frame_%06d.png"
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-framerate",
            str(fps),
            "-i",
            str(frame_pattern),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(output_path),
        ],
        check=True,
    )


def mux_audio(video_path: pathlib.Path, audio_path: pathlib.Path, output_path: pathlib.Path) -> None:
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-i",
            str(video_path),
            "-i",
            str(audio_path),
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-shortest",
            str(output_path),
        ],
        check=True,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Postprocess record frames into MP4 with narration")
    parser.add_argument("run_dir", help="Run directory containing frames and commentary log")
    parser.add_argument("--voice", default="Microsoft Zira Desktop", help="SAPI voice name")
    parser.add_argument("--fps", type=int, default=None, help="Override FPS if run.json missing")
    parser.add_argument("--skip-tts", action="store_true", help="Skip TTS synthesis")
    parser.add_argument("--skip-video", action="store_true", help="Skip frame->video step")
    args = parser.parse_args()

    run_dir = pathlib.Path(args.run_dir)
    run_json = run_dir / "run.json"
    fps = args.fps
    if run_json.exists():
        with run_json.open("r", encoding="utf-8") as handle:
            data = json.load(handle)
        fps = fps or int(data.get("fps", 0))
    if not fps:
        print("FPS missing; provide --fps or ensure run.json has fps", file=sys.stderr)
        return 1

    frames_dir = run_dir / "frames"
    if not frames_dir.exists():
        print(f"frames directory missing: {frames_dir}", file=sys.stderr)
        return 1

    commentary_txt = build_commentary_txt(run_dir)
    narration_wav = run_dir / "narration.wav"
    capture_mp4 = run_dir / "capture.mp4"
    final_mp4 = run_dir / "final.mp4"

    if not args.skip_video:
        build_video(frames_dir, fps, capture_mp4)
    if not args.skip_tts:
        synthesize_tts(commentary_txt, narration_wav, args.voice)
    if capture_mp4.exists() and narration_wav.exists():
        mux_audio(capture_mp4, narration_wav, final_mp4)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
