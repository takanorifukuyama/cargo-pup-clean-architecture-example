# cargo-pup 0.1.8 の実験記録

## 条件

- 調査日: 2026-09-11
- 実行: GitHub Actions / Ubuntu / x86_64
- 配布物: crates.io `cargo_pup = 0.1.8`（`cargo install --locked`）
- コンパイラ: `nightly-2026-01-22`
- アプリ: 外部crate依存なし、1ライブラリ内の4層

## 実行して判明したtraitルールの未実装

初回は「`TaskRepository`で終わるstructにport実装を必須とする」ルールを設定し、traitを実装しない`BrokenTaskRepository`が落ちることを期待した。

結果は、通常コンパイルもcargo-pupも成功した。テストハーネスはこれを見逃しとして正しく失敗させた。

[初回CIのログ](https://github.com/takanorifukuyama/cargo-pup-clean-architecture-example/actions/runs/34567189709/job/103161615337)に以下の流れが残っている。

```text
PASS: actual application respects the configured rules
PASS: domain -> infrastructure rejected by domain_no_outer_imports
PASS: application -> infrastructure rejected by application_no_adapters
PASS: presentation -> infrastructure rejected by presentation_no_infrastructure
PASS: infrastructure -> presentation rejected by infrastructure_no_presentation
PASS: nested domain -> presentation rejected by domain_no_outer_imports
violation was missed: repository without the application's port
```

その後、[StructRuleの定義](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_config/src/struct_lint/types.rs)と[検査実装](https://github.com/DataDog/cargo-pup/blob/a2c06497096123d4d37f622ddadb934831c60e92/cargo_pup_lint_impl/src/lints/struct_lint/lint.rs)を突き合わせた。

`StructRule::ImplementsTrait`はenumにはある。しかし、`StructLint::check_item`が処理するのは名前・可視性に関するルールであり、その他は`_ => {}`で無視される。

**原因は「traitを実装したかを必須条件として検査するルールの未実装」であり、サンプルの正規表現の調整で直る問題ではない。** なお、対象structを絞り込む`StructMatch::ImplementsTrait`は別の処理なので混同しない。

## サンプルへの反映

この失敗を隠すために単純にテストを削除せず、次のように区別した。

- 動作する設計制約: import制限と公開範囲。compile-validな違反例に対する名前付き診断を要求する。
- 観測用: `probe_repository_trait_requirement`として設定を残し、未実装で通ってしまうことをcharacterization testで記録する。設計を強制するルールとしては数えない。
- 必要な型保証: 実際にユースケースへ注入するRepositoryは、`CreateTask<R: TaskRepository>`によりRustコンパイラがtrait実装を要求する。

公開範囲ルールも同じ`Name(".*TaskRepository$")` matcherを使うため、命名パターンが対象structを選べることは別の違反fixtureで検証する。

## もう一つの既知の限界

`RestrictImports`は`use`のパスを扱う。型や式に完全修飾パスを直接書くケースには`use`がなく、現在のルールでは捉えられない。専用fixtureに悪い依存を残し、意図的な既知の限界として観測する。

いずれも「その設計を許可する」という意味ではない。更新時に挙動が変わったら診断を確認し、より強い保証として扱えるかを判断して期待値と説明を更新する。
