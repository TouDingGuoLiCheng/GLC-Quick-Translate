export interface BubbleBgLayout {
  posX: number;
  posY: number;
  zoom: number;
}

export interface BubbleBgLayouts {
  loading: BubbleBgLayout;
  success: BubbleBgLayout;
}

export interface TranslateSettings {
  enabled: boolean;
  /** 翻译并显示气泡 */
  hotkey: string;
  /** 翻译并用译文替换选中内容 */
  replaceHotkey: string;
  /** 气泡展示译文后，用译文替换原选中内容 */
  bubbleReplaceHotkey: string;
  restoreClipboard: boolean;
  copyDelayMs: number;
  /** 上一条成功翻译记录超过该秒数后，空选误读其原文/译文时提示空选；0=关闭（非等待） */
  translateClipboardGuardSec: number;
  targetLang: string;
  primaryProvider: string;
  fallbackEnabled: boolean;
  timeoutSec: number;
  cacheTtlSec: number;
  bubbleAutoCloseSec: number;
  usePythonCapture: boolean;
  /** 应用主题 dark | light */
  appTheme: string;
  /** 翻译气泡卡片不透明度 50–100 */
  bubbleOpacity: number;
  /** 气泡背景图文件名（空为默认） */
  bubbleBackground?: string;
  /** 各气泡形态的独立背景裁切 */
  bubbleBgLayouts?: BubbleBgLayouts;
  /** 气泡主文字色（空=跟随主题） */
  bubbleTextColor?: string;
  /** 气泡次要文字色（空=跟随主题） */
  bubbleMutedColor?: string;
  /** @deprecated 旧版统一裁切，载入时迁移到 bubbleBgLayouts */
  bubbleBgPosX?: number;
  bubbleBgPosY?: number;
  bubbleBgZoom?: number;
  /** 历史记录保留条数 20–500 */
  historyMaxCount?: number;
  /** 开机自动启动 */
  launchAtStartup?: boolean;
}

export interface SelectionResult {
  ok: boolean;
  text?: string;
  error?: string;
  restoredClipboard: boolean;
  durationMs: number;
  /** 取词时剪贴板序号已变化（复制确实发生） */
  clipboardSequenceChanged?: boolean;
}

export interface TranslateResult {
  ok: boolean;
  sourceText: string;
  translatedText?: string;
  provider?: string;
  fromCache: boolean;
  error?: string;
  durationMs: number;
}

export interface TranslateDonePayload {
  selection: SelectionResult;
  translate: TranslateResult;
}

export interface HistoryRecord {
  id: string;
  createdAt: number;
  sourceText: string;
  translatedText?: string;
  ok: boolean;
  error?: string;
  provider?: string;
  fromCache: boolean;
  durationMs: number;
}

export interface TranslatorProviderMeta {
  id: string;
  name: string;
  enabled: boolean;
  urlTemplate?: string;
  resultSelector?: string;
}

export interface TranslatorsFile {
  version: number;
  providers: TranslatorProviderMeta[];
}
