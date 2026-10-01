# Meeting engine sources and credits

The optional meeting engine downloads these components into the application's data directory. They are not checked into this repository.

| Component | Source / terms |
| --- | --- |
| sherpa-onnx 1.13.8 | [k2-fsa/sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), Apache-2.0; see installed package notices. |
| Pyannote segmentation 3.0 ONNX | [sherpa-onnx segmentation release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-segmentation-models), derived from [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0). The upstream model card supplies its MIT license and authorship. |
| WeSpeaker VoxCeleb ResNet34 ONNX | [sherpa-onnx speaker recognition release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/speaker-recongition-models); [upstream checkpoint](https://huggingface.co/Wespeaker/wespeaker-voxceleb-resnet34) and [WeSpeaker pretrained model documentation](https://github.com/wenet-e2e/wespeaker/blob/master/docs/pretrained.md). Consult both checkpoint and dataset terms when redistributing weights. |
| NumPy 2.2.6 | Installed into the private runtime by pip, including its bundled license files. |
| FFmpeg | Supplied locally or via PATH / `FERROFLUID_FFMPEG_BIN`. [FFmpeg license information](https://ffmpeg.org/legal.html). This repository does not distribute a new FFmpeg binary. |

Whisper and the existing bundled components retain the notices in `LICENSE-THIRD-PARTY.txt`.
