import Editor, { type OnMount } from "@monaco-editor/react";
import type * as monaco from "monaco-editor";
import { useCallback, useEffect, useRef, useState } from "react";

import { ipc, type TemplateFull } from "@/lib/ipc";

import { CONDUCTION_THEME, registerConductionLuaProvider } from "./monacoSetup";

const MARKER_OWNER = "conduction-lua";

interface ScriptEditorProps {
  template: TemplateFull;
  editable: boolean;
  /** Compile 結果を受け取って draftTracks に反映する。 */
  onCompileSuccess: (compiled: TemplateFull) => void;
}

const STARTER_SCRIPT = `-- Conduction Script Example
-- Sweeping crossfader from A to B over 32 beats.

set_duration(32)

add_keyframe("crossfader", 0,  -1, "linear")
add_keyframe("crossfader", 32,  1, "ease_in_out")

-- Decaying low-pass sweep on Deck A using math.sin.
for i = 0, 32, 4 do
  add_keyframe("deck_filter.A", i, math.sin(i / 16 * math.pi) * 0.5)
end
`;

export function ScriptEditor({
  template,
  editable,
  onCompileSuccess,
}: ScriptEditorProps) {
  const [source, setSource] = useState<string>(
    template.source ?? STARTER_SCRIPT,
  );
  const [error, setError] = useState<string | null>(null);
  const [compiling, setCompiling] = useState(false);
  const [lastCompiledAt, setLastCompiledAt] = useState<number | null>(null);

  // Monaco の editor / monaco instance への参照。マーカー操作用。
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof monaco | null>(null);

  // Compile エラーから { line, message } を抜き出して setModelMarkers に渡す。
  // Lua のエラーは `[string "..."]:LINE: MESSAGE` 形式。先頭の prefix を取り除いて
  // line と純粋なメッセージを取る。失敗時 (パース不能) は line 1 にまるごと貼る。
  const setEditorError = useCallback((raw: string | null) => {
    const editor = editorRef.current;
    const mn = monacoRef.current;
    if (!editor || !mn) return;
    const model = editor.getModel();
    if (!model) return;
    if (!raw) {
      mn.editor.setModelMarkers(model, MARKER_OWNER, []);
      return;
    }
    // 例: "Lua runtime error: [string \"<lua>\"]:5: attempt to call a nil value (global 'foo')"
    const m = /\[string ".*?"\]:(\d+):\s*([\s\S]*)$/.exec(raw)
      ?? /:(\d+):\s*([\s\S]*)$/.exec(raw);
    let lineNum = 1;
    let message = raw;
    if (m) {
      lineNum = Math.max(1, parseInt(m[1]!, 10));
      message = m[2]!.trim() || raw;
    }
    // 該当行の長さを取得して波線を引く範囲を決める
    const totalLines = model.getLineCount();
    const safeLine = Math.min(lineNum, totalLines);
    const lineContent = model.getLineContent(safeLine);
    mn.editor.setModelMarkers(model, MARKER_OWNER, [
      {
        severity: mn.MarkerSeverity.Error,
        message,
        startLineNumber: safeLine,
        startColumn: 1,
        endLineNumber: safeLine,
        endColumn: Math.max(2, lineContent.length + 1),
        source: "conduction-lua",
      },
    ]);
  }, []);

  useEffect(() => {
    setSource(template.source ?? STARTER_SCRIPT);
    setError(null);
    setEditorError(null);
    setLastCompiledAt(null);
  }, [template.id, setEditorError]);

  const handleCompile = useCallback(async () => {
    setCompiling(true);
    setError(null);
    setEditorError(null);
    try {
      const compiled = await ipc.compileLuaTemplate({
        source,
        default_duration_beats: template.duration_beats,
        template_id: template.id,
        template_name: template.name,
      });
      onCompileSuccess(compiled);
      setLastCompiledAt(Date.now());
    } catch (e) {
      const msg = String(e);
      setError(msg);
      setEditorError(msg);
    } finally {
      setCompiling(false);
    }
  }, [
    source,
    template.duration_beats,
    template.id,
    template.name,
    onCompileSuccess,
    setEditorError,
  ]);

  const handleMount: OnMount = useCallback((editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;
    registerConductionLuaProvider();
    // Cmd/Ctrl+S は OS の保存と紛らわしいので、Cmd/Ctrl+Enter で Compile。
    editor.addCommand(
      // eslint-disable-next-line no-bitwise
      monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter,
      () => {
        void handleCompile();
      },
    );
  }, [handleCompile]);

  return (
    <div className="script-editor">
      <div className="script-editor-toolbar">
        <button
          type="button"
          className="btn"
          data-variant="primary"
          onClick={() => void handleCompile()}
          disabled={!editable || compiling}
          title="Compile (⌘/Ctrl + Enter)"
        >
          {compiling ? "Compiling…" : "Compile"}
        </button>
        <span className="hint" style={{ fontSize: "var(--fs-micro)" }}>
          Globals: <code>duration_beats</code> · <code>set_duration(n)</code> ·{" "}
          <code>add_keyframe(target, beat, value [, curve])</code> ·{" "}
          <code>add_track(target, table)</code>
          {lastCompiledAt && (
            <> · Compiled OK — open Visual / Node to inspect, Save to persist</>
          )}
        </span>
      </div>
      <div className="script-editor-monaco">
        <Editor
          height="100%"
          language="lua"
          theme={CONDUCTION_THEME}
          value={source}
          onChange={(v) => setSource(v ?? "")}
          onMount={handleMount}
          options={{
            readOnly: !editable,
            // フォント未指定で Monaco デフォルトに任せる (suggest widget の text が
            // 消える問題の切り分け)。
            fontFamily: "Menlo, Monaco, Consolas, monospace",
            fontSize: 13,
            tabSize: 2,
            insertSpaces: true,
            minimap: { enabled: false },
            wordWrap: "on",
            scrollBeyondLastLine: false,
            renderWhitespace: "selection",
            quickSuggestions: { other: true, comments: false, strings: true },
            suggestOnTriggerCharacters: true,
            automaticLayout: true,
          }}
        />
      </div>
      {error && <pre className="script-editor-error">{error}</pre>}
    </div>
  );
}
