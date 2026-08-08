# 第三方声明

Atha 包含以下第三方资产。它们保留各自的版权与许可证，不因 Atha 采用 AGPL-3.0-or-later 而被重新许可。

## Markdown / TXT 直接依赖

- `pulldown-cmark 0.13.4`：MIT；Copyright 2015 Google Inc.；上游：<https://crates.io/crates/pulldown-cmark/0.13.4>
- `chardetng 1.0.0`：Apache-2.0 OR MIT；Copyright Mozilla Foundation；上游：<https://crates.io/crates/chardetng/1.0.0>
- `encoding_rs 0.8.35`：`(Apache-2.0 OR MIT) AND BSD-3-Clause`；Copyright Mozilla Foundation；其 WHATWG 数据版权归 Apple、Google、Mozilla、Microsoft 组成的 WHATWG；上游：<https://crates.io/crates/encoding_rs/0.8.35>
- `regex 1.13.1`：MIT OR Apache-2.0；Copyright 2014 The Rust Project Developers；上游：<https://crates.io/crates/regex/1.13.1>

Apache-2.0 全文见 `LICENSES/Apache-2.0.txt`；MIT 全文见本文末尾。`encoding_rs` 所含 WHATWG 数据的 BSD-3-Clause 声明如下：

```text
Copyright © WHATWG (Apple, Google, Mozilla, Microsoft).

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Gradle Wrapper 8.14.3

- 本地文件：`reader/app/src-tauri/gen/android/gradle/wrapper/gradle-wrapper.jar`
- 上游源码：[`gradle/gradle` v8.14.3 wrapper-main](https://github.com/gradle/gradle/tree/v8.14.3/platforms/core-runtime/wrapper-main)
- 许可证：Apache-2.0；全文见 `LICENSES/Apache-2.0.txt`
- 本地修改：无；由 Tauri Android 工程生成器写入

## Microsoft Fluent System Icons

- 本地文件：`reader/assets/bookmark-24-regular.svg`
- 上游文件：[`ic_fluent_bookmark_24_regular.svg`](https://github.com/microsoft/fluentui-system-icons/blob/9e9a1766ae48f4a138fed896b25a59a5f6619230/assets/Bookmark/SVG/ic_fluent_bookmark_24_regular.svg)
- 上游版本：`9e9a1766ae48f4a138fed896b25a59a5f6619230`
- 许可证：MIT
- 本地修改：仅调整 XML 空白格式，不改变元素、属性或路径数据

以下版权与许可文本原样取自该版本的上游 `LICENSE`：

```text
MIT License

Copyright (c) 2020 Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
