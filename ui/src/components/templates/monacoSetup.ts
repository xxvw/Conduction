// Monaco editor のオフライン初期化。
// - bundled monaco-editor を @monaco-editor/react に渡して CDN フェッチを無効化
// - Lua は専用 worker を持たず、editor.worker も必須ではない。
//   ?worker import を使うと Vite/WebKit 環境で suggest widget の描画が壊れる
//   症状が出たので、worker は使わず main thread で動かす
//   (小規模スクリプトしか書かないので問題なし)。
// - Conduction Lua API の補完プロバイダを登録

import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
// 既存の Lua Monarch 定義を取り出して上書き拡張する。
// 型は `ui/src/monaco-shims.d.ts` で declare module している。
import {
  language as luaLanguage,
  conf as luaConf,
} from "monaco-editor/esm/vs/basic-languages/lua/lua.js";

// @monaco-editor/react に bundled instance を渡す。これで CDN フェッチをスキップ。
loader.config({ monaco });

// Editor の初期 mount 時には theme / token provider が登録済みであってほしいので、
// loader.init() を待たずに module-load 時に即時登録する。
// (monaco は bundled instance なのでこの時点で完全に使える)
queueMicrotask(() => {
  try {
    registerConductionLuaProvider();
  } catch {
    /* setMonarchTokensProvider が二重に走るのは想定済み (_registered guard) */
  }
});

// ---- Conduction Lua API completion provider ----

const TARGETS: ReadonlyArray<{ key: string; doc: string }> = [
  { key: "crossfader", doc: "Crossfader. Range -1 (A) to +1 (B)." },
  { key: "master_volume", doc: "Master output volume. Range 0 to 2." },
  { key: "deck_volume.A", doc: "Deck A channel volume. Range 0 to 2." },
  { key: "deck_volume.B", doc: "Deck B channel volume. Range 0 to 2." },
  { key: "deck_eq_low.A", doc: "Deck A EQ Low (dB). Range -26 to +6." },
  { key: "deck_eq_low.B", doc: "Deck B EQ Low (dB). Range -26 to +6." },
  { key: "deck_eq_mid.A", doc: "Deck A EQ Mid (dB). Range -26 to +6." },
  { key: "deck_eq_mid.B", doc: "Deck B EQ Mid (dB). Range -26 to +6." },
  { key: "deck_eq_high.A", doc: "Deck A EQ High (dB). Range -26 to +6." },
  { key: "deck_eq_high.B", doc: "Deck B EQ High (dB). Range -26 to +6." },
  { key: "deck_filter.A", doc: "Deck A Filter. Range -1 (LP) to +1 (HP)." },
  { key: "deck_filter.B", doc: "Deck B Filter. Range -1 (LP) to +1 (HP)." },
  { key: "deck_echo_wet.A", doc: "Deck A Echo wet. Range 0 to 1." },
  { key: "deck_echo_wet.B", doc: "Deck B Echo wet. Range 0 to 1." },
  { key: "deck_reverb_wet.A", doc: "Deck A Reverb wet. Range 0 to 1." },
  { key: "deck_reverb_wet.B", doc: "Deck B Reverb wet. Range 0 to 1." },
];

const CURVES = [
  "linear",
  "ease_in",
  "ease_out",
  "ease_in_out",
  "step",
  "hold",
] as const;

let _registered = false;

/** Conduction 拡張テーマ名。Monaco の vs-dark を継承して独自トークンを色付け。 */
export const CONDUCTION_THEME = "conduction-dark";

export function registerConductionLuaProvider() {
  if (_registered) return;
  _registered = true;

  // ---- Lua Monarch tokenizer を拡張 (Conduction の関数 / 定数 / target / curve を別 token に) ----
  const extendedLua: monaco.languages.IMonarchLanguage = {
    ...luaLanguage,
    tokenizer: {
      ...luaLanguage.tokenizer,
      // root の先頭に独自ルールを差し込み、Lua 既存のルールはあとで適用させる。
      root: [
        // 関数 (set_duration / add_keyframe / add_track) と prelude ヘルパ
        [
          /\b(?:set_duration|add_keyframe|add_track|clamp|lerp|smoothstep|each_bar|each_phrase)\b/,
          "keyword.conduction",
        ],
        // 定数 (duration_beats)
        [/\bduration_beats\b/, "constant.conduction"],
        // target 文字列リテラル
        [
          /"(?:crossfader|master_volume|deck_(?:volume|eq_low|eq_mid|eq_high|filter|echo_wet|reverb_wet)\.[AB])"/,
          "string.target.conduction",
        ],
        // curve 文字列リテラル
        [
          /"(?:linear|ease_in|ease_out|ease_in_out|step|hold)"/,
          "string.curve.conduction",
        ],
        ...(luaLanguage.tokenizer.root as monaco.languages.IMonarchLanguageRule[]),
      ],
    },
  };
  monaco.languages.setMonarchTokensProvider("lua", extendedLua);
  monaco.languages.setLanguageConfiguration("lua", luaConf);

  // ---- 拡張テーマ (vs-dark をベースに Conduction tokens を着色) ----
  monaco.editor.defineTheme(CONDUCTION_THEME, {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "keyword.conduction", foreground: "4FE3B2", fontStyle: "bold" },
      { token: "constant.conduction", foreground: "FFC547", fontStyle: "bold" },
      // target は target color (deck) と被らない accent green、curve は subtle blue
      { token: "string.target.conduction", foreground: "7AE655" },
      { token: "string.curve.conduction", foreground: "8A9BE8", fontStyle: "italic" },
    ],
    colors: {},
  });

  monaco.languages.registerCompletionItemProvider("lua", {
    triggerCharacters: ['"', "'", ".", "(", ","],
    provideCompletionItems: (model, position) => {
      const lineUpToCursor = model.getValueInRange({
        startLineNumber: position.lineNumber,
        startColumn: 1,
        endLineNumber: position.lineNumber,
        endColumn: position.column,
      });

      const word = model.getWordUntilPosition(position);
      const range: monaco.IRange = {
        startLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endLineNumber: position.lineNumber,
        endColumn: word.endColumn,
      };

      const suggestions: monaco.languages.CompletionItem[] = [];
      const ItemKind = monaco.languages.CompletionItemKind;

      // 文字列リテラル内の場合、target / curve を提案する。
      const inString = isInsideString(lineUpToCursor);
      if (inString) {
        const ctx = detectStringContext(lineUpToCursor);
        if (ctx === "target") {
          for (const t of TARGETS) {
            suggestions.push({
              label: t.key,
              kind: ItemKind.Value,
              insertText: t.key,
              documentation: t.doc,
              range,
            });
          }
        } else if (ctx === "curve") {
          for (const c of CURVES) {
            suggestions.push({
              label: c,
              kind: ItemKind.Value,
              insertText: c,
              range,
            });
          }
        } else {
          // 文脈不明: target を出しておく (多くの場合 1 つ目の引数 = target)
          for (const t of TARGETS) {
            suggestions.push({
              label: t.key,
              kind: ItemKind.Value,
              insertText: t.key,
              documentation: t.doc,
              range,
            });
          }
        }
        return { suggestions };
      }

      // 通常文脈: Conduction API + Lua 言語の主要キーワード / 制御構造を提案。
      // スニペットの choice 構文 (${1|a,b|}) は Monaco のバージョンによっては popup
      // の rendering を壊すので、placeholder ${1:foo} だけ使う。
      const SnippetRule =
        monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet;

      // --- Conduction API ---
      suggestions.push(
        {
          label: "set_duration",
          kind: ItemKind.Function,
          insertText: "set_duration(${1:32})",
          insertTextRules: SnippetRule,
          documentation:
            "set_duration(beats: number) — テンプレート全長 (拍) を設定する。",
          detail: "(beats: number)",
          range,
        },
        {
          label: "add_keyframe",
          kind: ItemKind.Function,
          insertText:
            'add_keyframe("${1:crossfader}", ${2:0}, ${3:0}, "${4:linear}")',
          insertTextRules: SnippetRule,
          documentation:
            "add_keyframe(target, beat, value [, curve]) — target に keyframe を 1 つ追加。",
          detail: "(target, beat, value [, curve])",
          range,
        },
        {
          label: "add_track",
          kind: ItemKind.Function,
          insertText:
            'add_track("${1:crossfader}", {\n  {beat = ${2:0}, value = ${3:-1}, curve = "${4:linear}"},\n  {beat = ${5:16}, value = ${6:1}, curve = "${7:linear}"},\n})',
          insertTextRules: SnippetRule,
          documentation:
            "add_track(target, keyframes_table) — keyframes をまとめて 1 つの target に追加。",
          detail: "(target, table)",
          range,
        },
        {
          label: "duration_beats",
          kind: ItemKind.Variable,
          insertText: "duration_beats",
          documentation: "テンプレート全長 (拍)。set_duration で変更可能。",
          detail: "number",
          range,
        },
      );

      // --- Prelude ヘルパ (Lua 側で定義済みのユーティリティ) ---
      suggestions.push(
        {
          label: "clamp",
          kind: ItemKind.Function,
          insertText: "clamp(${1:x}, ${2:min}, ${3:max})",
          insertTextRules: SnippetRule,
          documentation: "clamp(x, min, max) — x を min..max に丸めて返す。",
          detail: "(x, min, max) — prelude",
          range,
        },
        {
          label: "lerp",
          kind: ItemKind.Function,
          insertText: "lerp(${1:a}, ${2:b}, ${3:t})",
          insertTextRules: SnippetRule,
          documentation:
            "lerp(a, b, t) — 線形補間。t は 0..1 に clamp される。",
          detail: "(a, b, t) — prelude",
          range,
        },
        {
          label: "smoothstep",
          kind: ItemKind.Function,
          insertText: "smoothstep(${1:a}, ${2:b}, ${3:t})",
          insertTextRules: SnippetRule,
          documentation:
            "smoothstep(a, b, t) — Hermite smooth で a..b を補間。",
          detail: "(a, b, t) — prelude",
          range,
        },
        {
          label: "each_bar",
          kind: ItemKind.Function,
          insertText:
            "each_bar(function(beat, bar)\n\t$0\nend)",
          insertTextRules: SnippetRule,
          documentation:
            "each_bar(callback) — 4 拍ごとに callback(beat, bar) を呼ぶ。",
          detail: "(callback) — prelude",
          range,
        },
        {
          label: "each_phrase",
          kind: ItemKind.Function,
          insertText:
            "each_phrase(${1:8}, function(beat, phrase)\n\t$0\nend)",
          insertTextRules: SnippetRule,
          documentation:
            "each_phrase(beats_per_phrase, callback) — 指定拍数ごとに反復。",
          detail: "(beats_per_phrase, callback) — prelude",
          range,
        },
      );

      // --- Lua 制御構造スニペット ---
      suggestions.push(
        {
          label: "for",
          kind: ItemKind.Snippet,
          insertText: "for ${1:i} = ${2:0}, ${3:N} do\n\t$0\nend",
          insertTextRules: SnippetRule,
          documentation: "for i = start, stop [, step] do ... end",
          detail: "for loop",
          range,
        },
        {
          label: "forin",
          kind: ItemKind.Snippet,
          insertText: "for ${1:k}, ${2:v} in pairs(${3:t}) do\n\t$0\nend",
          insertTextRules: SnippetRule,
          documentation: "for k, v in pairs(t) do ... end",
          detail: "for...in loop",
          range,
        },
        {
          label: "if",
          kind: ItemKind.Snippet,
          insertText: "if ${1:cond} then\n\t$0\nend",
          insertTextRules: SnippetRule,
          documentation: "if cond then ... end",
          detail: "if statement",
          range,
        },
        {
          label: "ifelse",
          kind: ItemKind.Snippet,
          insertText:
            "if ${1:cond} then\n\t${2}\nelse\n\t${3}\nend",
          insertTextRules: SnippetRule,
          documentation: "if cond then ... else ... end",
          detail: "if/else statement",
          range,
        },
        {
          label: "while",
          kind: ItemKind.Snippet,
          insertText: "while ${1:cond} do\n\t$0\nend",
          insertTextRules: SnippetRule,
          documentation: "while cond do ... end",
          detail: "while loop",
          range,
        },
        {
          label: "function",
          kind: ItemKind.Snippet,
          insertText:
            "local function ${1:name}(${2})\n\t$0\nend",
          insertTextRules: SnippetRule,
          documentation: "local function name(args) ... end",
          detail: "function declaration",
          range,
        },
        {
          label: "local",
          kind: ItemKind.Snippet,
          insertText: "local ${1:name} = ${2}",
          insertTextRules: SnippetRule,
          documentation: "local variable declaration",
          detail: "local",
          range,
        },
      );

      // --- Lua keyword / builtin (plain text completion) ---
      const KEYWORDS: ReadonlyArray<string> = [
        "do",
        "end",
        "then",
        "else",
        "elseif",
        "return",
        "break",
        "nil",
        "true",
        "false",
        "and",
        "or",
        "not",
        "in",
        "repeat",
        "until",
      ];
      for (const kw of KEYWORDS) {
        suggestions.push({
          label: kw,
          kind: ItemKind.Keyword,
          insertText: kw,
          range,
        });
      }

      // --- math.* の主要関数 (Lua の `math` モジュール) ---
      const MATH_FNS: ReadonlyArray<string> = [
        "abs",
        "ceil",
        "floor",
        "min",
        "max",
        "sin",
        "cos",
        "tan",
        "asin",
        "acos",
        "atan",
        "sqrt",
        "pow",
        "exp",
        "log",
        "random",
        "pi",
        "huge",
      ];
      for (const fn of MATH_FNS) {
        suggestions.push({
          label: `math.${fn}`,
          kind: fn === "pi" || fn === "huge"
            ? ItemKind.Constant
            : ItemKind.Function,
          insertText: `math.${fn}`,
          range,
        });
      }

      return { suggestions };
    },
  });

  // Hover doc も同じソースから出す (target キーの解説など)
  monaco.languages.registerHoverProvider("lua", {
    provideHover: (model, position) => {
      const word = model.getWordAtPosition(position);
      if (!word) return null;
      const fns: Record<string, string> = {
        set_duration:
          "`set_duration(beats: number)` — テンプレート全長 (拍) を設定する。",
        add_keyframe:
          "`add_keyframe(target, beat, value [, curve])` — target に keyframe を 1 つ追加。",
        add_track:
          "`add_track(target, keyframes_table)` — keyframes をまとめて 1 つの target に追加。",
        duration_beats: "テンプレート全長 (拍)。`set_duration` で変更可能。",
        clamp: "`clamp(x, min, max)` — x を min..max に丸めて返す。",
        lerp: "`lerp(a, b, t)` — 線形補間。t は 0..1 に clamp される。",
        smoothstep:
          "`smoothstep(a, b, t)` — Hermite smooth (3t² - 2t³) で a..b を補間。",
        each_bar:
          "`each_bar(callback)` — 4 拍 (1 bar) ごとに callback(beat, bar_index) を呼ぶ。",
        each_phrase:
          "`each_phrase(beats_per_phrase, callback)` — 指定拍数ごとに callback(beat, phrase_index) を呼ぶ。",
      };
      const doc = fns[word.word];
      if (!doc) return null;
      return {
        contents: [{ value: doc }],
        range: new monaco.Range(
          position.lineNumber,
          word.startColumn,
          position.lineNumber,
          word.endColumn,
        ),
      };
    },
  });
}

// カーソル直前までの行を見て、開いた " / ' があるかでざっくり判定。
function isInsideString(lineUpToCursor: string): boolean {
  let quote: '"' | "'" | null = null;
  let escaped = false;
  for (const ch of lineUpToCursor) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      continue;
    }
    if (quote === null && (ch === '"' || ch === "'")) {
      quote = ch;
    } else if (quote === ch) {
      quote = null;
    }
  }
  return quote !== null;
}

// 直近の関数呼び出し名 + 何番目の引数かをざっくり推定して、
// target を提案するか curve を提案するか決める。
function detectStringContext(
  lineUpToCursor: string,
): "target" | "curve" | null {
  // 最後の `(` から後ろを切り出して引数番号を数える (簡易)。
  const lastParen = lineUpToCursor.lastIndexOf("(");
  if (lastParen < 0) return null;
  const before = lineUpToCursor.slice(0, lastParen);
  // 関数名: ( の直前の word
  const fnMatch = /([a-zA-Z_][a-zA-Z0-9_]*)\s*$/.exec(before);
  if (!fnMatch) return null;
  const fn = fnMatch[1]!;
  const args = lineUpToCursor.slice(lastParen + 1);
  const argIdx = countTopLevelCommas(args);
  if (fn === "add_keyframe") {
    if (argIdx === 0) return "target";
    if (argIdx === 3) return "curve";
    return null;
  }
  if (fn === "add_track") {
    if (argIdx === 0) return "target";
    return null;
  }
  return null;
}

// ネスト ( や { を尊重しつつ , の数を数える。
function countTopLevelCommas(s: string): number {
  let depth = 0;
  let commas = 0;
  let quote: '"' | "'" | null = null;
  let escaped = false;
  for (const ch of s) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      continue;
    }
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      continue;
    }
    if (ch === "(" || ch === "{" || ch === "[") depth++;
    else if (ch === ")" || ch === "}" || ch === "]") depth--;
    else if (ch === "," && depth === 0) commas++;
  }
  return commas;
}
