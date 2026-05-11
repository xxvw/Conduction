// monaco-editor の basic-languages サブパスには型定義が同梱されていないため
// 自前で declare する。runtime では `language` (Monarch grammar) と `conf`
// (language configuration) を export している。
declare module "monaco-editor/esm/vs/basic-languages/lua/lua.js" {
  import type * as monaco from "monaco-editor";
  export const language: monaco.languages.IMonarchLanguage;
  export const conf: monaco.languages.LanguageConfiguration;
}
