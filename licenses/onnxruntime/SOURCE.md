# Where these two files come from

`LICENSE` and `ThirdPartyNotices.txt` are copied unchanged from Microsoft's NuGet package
`Microsoft.ML.OnnxRuntime` 1.31.0:

- <https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.31.0/microsoft.ml.onnxruntime.1.31.0.nupkg>
- SHA-256 of the package: `7e35fdb4f5adb62f01e60798e2e5ed05108f5fbbe1cca03fbcc37903cf7c5e3b`
- Source commit named in its `.nuspec`: `402979ac1bef89dcd55add41eaa7277c407599eb`

The program links the ONNX Runtime 1.31.0 library that `ort-sys` 2.0.0-rc.15 downloads
(`build/download/dist.tsv`: `ms@1.31.0`). That download carries no licence file. It is built
by the `ort` project, not by Microsoft's NuGet pipeline, so it is not proven that the two
builds contain exactly the same third-party components.

When `ort-sys` changes, replace both files, then update `ORT_SYS` and `ORT_VERSION` in
`.github/scripts/third_party_licenses.py`; the script fails until they match.
