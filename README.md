# Compiler Judge

仓库收集了一些测试用例，同时提供了测试程序来方便地批量进行测试。`compiler_judge` 已经在按照 `FudanCompilerH2025/HW1/docs/lab1.md` 正确配置的 Ubuntu 20.04 环境下测试，确保可以正常运行。

> [!IMPORTANT]
> 如果您想到了任何测试用例，欢迎[贡献](#贡献测试用例)！

## 配置

1. Clone 本仓库，并把它放在 `FudanCompilerH2025` 目录下；
2. 在 GitHub 仓库页右侧找到 Releases，在其中下载 `compiler_judge` 可执行文件，并放入 `FudanCompilerH2025/compiler_judge` 目录（如果已安装 `Cargo`，可自己编译 `crates/compiler_judge` 下的项目；如果没安装，请忽略括号里的这句话）；
3. 如果执行了第二步，请在 `FudanCompilerH2025/compiler_judge` 目录下执行 `chmod +x ./compiler_judge` 来给 `compiler_judge` 可执行权限。

完成以上三步后，目录结构大致如下：

```
FudanCompilerH2025
|-- HW1
|-- HW2
|-- compiler_judge （Clone 下来的仓库）
|   |-- HW1
|   |   |-- index.toml
|   |   |-- ...
|   |-- compiler_judge （从 Release 页下载的可执行文件）
|   |-- ...
|-- ...
```

## 基本用法

> [!WARNING]
> 请注意，如果希望使用标准输出的结果来判定正确性，请您不要自己的程序中包含除了输出答案之外的 `cout` 调用，否则这些额外的输出会被当做错误答案。
> 如果您希望输出调试信息，请使用 `cerr`。
>
> 例如，在 HW1 中，只应输出 `execute` 得到的最终答案。

基本用法：

```bash
./compiler_judge/compiler_judge <测试用例所在的目录>
```

例如，如果想测试 HW1，可以使用如下命令（附带了我执行的结果）：

```bash
$ ./compiler_judge/compiler_judge ./compiler_judge/HW1

Test: sample_3    [ AC ]
Test: division_by_zero    [ RE ]
Test: is_your_program_slow    [ TL ]
Test: sample_1    [ AC ]
Test: deepseek_test_1    [ AC ]
Test: sample_4    [ AC ]
Test: sample_post_order    [ AC ]
Test: simple_test_1    [ AC ]
Test: signed_overflow    [ AC ]
Test: sample_2    [ AC ]

Summary:

  Accepted (8/10):
    sample_3: passed
    sample_1: passed
    deepseek_test_1: passed
    sample_4: passed
    sample_post_order: passed
    simple_test_1: passed
    signed_overflow: passed
    sample_2: passed

  Time Limit Exceeded (1/10):
    is_your_program_slow: timeout after 2s

  Runtime Error (1/10):
    division_by_zero: runtime error: Process exited with non-zero code
```

每个测试用例有五种可能的执行结果：

- **Accepted (AC)**: 表示答案正确；
- **Manual (MN)**：您需要自行判断答案的正确性；
- **Wrong Answer (WA)**：表示答案错误；
- **Time Limit Exceeded (TL)**：表示执行超时；
- **Runtime Error (RE)**：表示运行时发生错误，这里的错误是您的程序产生的错误，而不是 `compiler_judge` 产生的。

## 测试用例格式

您可以自己增加或删除测试用例。一组测试用例放在一个包含 `index.toml` 文件的目录中，会一起进行测试。例如，上面的例子中测试用例都放在 `FudanCompilerH2025/compiler_judge/HW1` 中，
您可以在其中看到一个 `index.toml` 文件。

`index.toml` 文件包含了若干个测试用例的信息：

```toml
[测试用例名称]
cmd = "要执行的命令"
input = "要额外向程序的标准输入（stdin）输入的内容"  # 可选项
output = "您的程序输出结果的位置"                   # 可选项，如果没有，则会读取标准输出 stdout
answer = "正确答案文件所在的位置"                   # 可选项，如果没有，则不会进行答案的比对
```

例如，这是 `FudanCompilerH2025/compiler_judge/HW1/index.toml` 中声明的一个测试用例：

```toml
[sample_1]
cmd = "../../HW1/build/tools/main/main ../../HW1/test/test1"
answer = "./test1.out"
```

需要注意的是，**这里的路径全部以当前 `index.toml` 所在的目录为基准**。

## 额外配置

### 超时时间

默认情况下，如果一个测试用例执行时间超过了 2 秒，就会被判定为超时。您可以通过环境变量 `COMPILER_JUDGE_TIMEOUT` 自己修改这个限制。
例如下面这个例子将时间限制修改为 10 秒：

```bash
COMPILER_JUDGE_TIMEOUT=10 ./compiler_judge/compiler_judge ./compiler_judge/HW1
```

### 同时测试数量

默认情况下，`compiler_judge` 会同时进行 4 个测试，这是为了提高测试效率。您可以通过环境变量 `COMPILER_JUDGE_NJOBS` 自己修改这个限制。
例如下面这个例子允许同时进行 8 个测试：

```bash
COMPILER_JUDGE_NJOBS=8 ./compiler_judge/compiler_judge ./compiler_judge/HW1
```

## 贡献测试用例

请将测试用例放在对应作业所在的目录中。例如，如果要贡献 HW1 的名叫 `corner_case_1` 的测试用例，以下是推荐的文件存放位置：

- `FudanCompilerH2025/compiler_judge/HW1/corner_case_1.fmj`：您编写的 FDMJ-SLP 源代码
- `FudanCompilerH2025/compiler_judge/HW2/corner_case_1.out`：这个测试用例的正确答案

可以在 `index.toml` 中添加声明，这样之后就可以运行这个测试用例了：

```toml
[corner_case_1]
cmd = "../../HW1/build/tools/main/main ./corner_case_1"
answer = "./corner_case_1.out"
```

正如之前所说的，有的测试可能是为了测试程序是否崩溃（Runtime Error），所以没有正确答案也行。

然后您就可以发起 pull request 了！
