import type { Language } from "./types";

export interface Translations {
  settingsTitle: string;
  settingsSubtitle: string;
  localModelSec: string;
  modelPath: string;
  whisperEngine: string;
  changeModelBtn: string;
  openFolderBtn: string;
  downloadSec: string;
  selectLanguage: string;
  qualityProfile: string;
  computeBackend: string;
  
  // App Language Options
  langAuto: string;
  langEn: string;
  langRu: string;
  langUk: string;
  langZh: string;
  langEs: string;

  // Quality Profile Options
  qualityFast: string;
  qualityBalanced: string;
  qualityAccurate: string;

  // Widget Messages / status hints
  hintSelectModel: string;
  hintEngineMissing: string;
  hintCopied: string;
  hintCheckSettings: string;
  hintReady: string;
  
  // Tooltips
  tooltipCopy: string;
  tooltipSettings: string;
  tooltipClose: string;

  // Exit Modal
  exitTitle: string;
  exitConfirm: string;
  exitCancel: string;
  exitSubmit: string;
  
  // Other Settings
  modelFound: string;
  modelMissing: string;
  loadingModels: string;
  loading: string;
  themeDark: string;
  themeLight: string;

  // Settings Tabs
  tabModels: string;
  tabHotkeys: string;
  tabGeneral: string;

  // Hotkeys & Automation
  hotkeySec: string;
  opMode: string;
  opModeAlways: string;
  opModeHold: string;
  activationHotkey: string;
  hotkeyPressPrompt: string;
  hotkeyRecorded: string;
  hotkeyCancelled: string;
  hotkeyRecordBtn: string;
  autoSubmit: string;
  autoSubmitDisabled: string;
  autoSubmitEnabled: string;
  leftRightMouseWarning: string;

  // Model actions
  btnSelected: string;
  btnUse: string;
  btnDownload: string;
  btnCancel: string;
  btnDeleteTooltip: string;

  // Model descriptions
  modelDescTiny: string;
  modelDescBase: string;
  modelDescSmall: string;
  modelDescMedium: string;
  modelDescLargeV3Turbo: string;
  modelDescLargeV3: string;

  msgCopied: string;
  msgTranscriptReady: string;
  msgRecordDidNotStart: string;
  msgChooseModel: string;
  msgEngineMissing: string;
  msgRecordingFailed: string;
  hotkeyUnassigned: string;
}

export const translations: Record<"en" | "ru" | "uk" | "zh" | "es", Translations> = {
  en: {
    settingsTitle: "Settings",
    settingsSubtitle: "Model and transcription preferences",
    localModelSec: "Local model",
    modelPath: "Active model",
    whisperEngine: "Whisper engine",
    changeModelBtn: "Change model",
    openFolderBtn: "Open models folder",
    downloadSec: "Download Whisper models",
    selectLanguage: "Selected language",
    qualityProfile: "Quality profile",
    computeBackend: "Compute backend",
    langAuto: "Auto",
    langEn: "English",
    langRu: "Russian",
    langUk: "Ukrainian",
    langZh: "Chinese",
    langEs: "Spanish",
    qualityFast: "Fast",
    qualityBalanced: "Balanced",
    qualityAccurate: "Accurate",
    hintSelectModel: "Select model in settings",
    hintEngineMissing: "Engine missing",
    hintCopied: "Copied.",
    hintCheckSettings: "Check settings.",
    hintReady: "Ferrofluid Voice",
    tooltipCopy: "Copy transcript",
    tooltipSettings: "Settings",
    tooltipClose: "Close",
    exitTitle: "Exit application?",
    exitConfirm: "Are you sure you want to close the app?",
    exitCancel: "Cancel",
    exitSubmit: "Exit",
    modelFound: "Model found",
    modelMissing: "Model missing",
    loadingModels: "Loading models...",
    loading: "Loading...",
    themeDark: "Dark",
    themeLight: "Light",

    // Settings translations
    tabModels: "Model Management",
    tabHotkeys: "Hotkeys & Dictation",
    tabGeneral: "General Settings",
    hotkeySec: "Hotkeys & Automation",
    opMode: "Operation Mode",
    opModeAlways: "Always On Screen (Always On)",
    opModeHold: "Hold Hotkey to Record",
    activationHotkey: "Activation Hotkey",
    hotkeyPressPrompt: "Press key or click mouse...",
    hotkeyRecorded: "Recorded: {name}!",
    hotkeyCancelled: "Cancelled",
    hotkeyRecordBtn: "Record",
    autoSubmit: "Auto-Submit (Enter)",
    autoSubmitDisabled: "Disabled",
    autoSubmitEnabled: "Simulate Enter after insert",
    leftRightMouseWarning: "Left and right mouse clicks are protected to prevent interface locking.",
    btnSelected: "Selected",
    btnUse: "Use",
    btnDownload: "Download",
    btnCancel: "Cancel",
    btnDeleteTooltip: "Delete model from disk",

    // Model descriptions
    modelDescTiny: "Fastest option for short notes and quick commands.",
    modelDescBase: "Good default for lightweight local transcription.",
    modelDescSmall: "Better accuracy while still practical on most laptops.",
    modelDescMedium: "Higher quality for longer dictation and mixed speech.",
    modelDescLargeV3Turbo: "Strong quality with better speed than the full large model.",
    modelDescLargeV3: "Best quality option, needs more disk and compute.",

    msgCopied: "Copied.",
    msgTranscriptReady: "Transcript ready.",
    msgRecordDidNotStart: "Recording did not start.",
    msgChooseModel: "Choose a local Whisper model in settings.",
    msgEngineMissing: "Whisper engine is missing. Add whisper-cli.exe in binaries.",
    msgRecordingFailed: "Recording failed.",
    hotkeyUnassigned: "Unassigned",
  },
  ru: {
    settingsTitle: "Настройки",
    settingsSubtitle: "Параметры модели и распознавания",
    localModelSec: "Локальная модель",
    modelPath: "Активная модель",
    whisperEngine: "Движок Whisper",
    changeModelBtn: "Сменить модель",
    openFolderBtn: "Папка с моделями",
    downloadSec: "Загрузить модели Whisper",
    selectLanguage: "Язык распознавания",
    qualityProfile: "Профиль качества",
    computeBackend: "Вычислительный бэкенд",
    langAuto: "Авто",
    langEn: "Английский",
    langRu: "Русский",
    langUk: "Украинский",
    langZh: "Китайский",
    langEs: "Испанский",
    qualityFast: "Быстрый",
    qualityBalanced: "Сбалансированный",
    qualityAccurate: "Точный",
    hintSelectModel: "Выберите модель в настройках",
    hintEngineMissing: "Движок отсутствует",
    hintCopied: "Скопировано.",
    hintCheckSettings: "Проверьте настройки.",
    hintReady: "Ferrofluid Voice",
    tooltipCopy: "Скопировать текст",
    tooltipSettings: "Настройки",
    tooltipClose: "Закрыть",
    exitTitle: "Закрыть приложение?",
    exitConfirm: "Вы действительно хотите закрыть приложение?",
    exitCancel: "Отмена",
    exitSubmit: "Выйти",
    modelFound: "Модель найдена",
    modelMissing: "Модель отсутствует",
    loadingModels: "Загрузка моделей...",
    loading: "Загрузка...",
    themeDark: "Темная",
    themeLight: "Светлая",

    // Settings translations
    tabModels: "Управление моделями",
    tabHotkeys: "Горячие клавиши",
    tabGeneral: "Основные настройки",
    hotkeySec: "Горячие клавиши и автоматизация",
    opMode: "Режим работы",
    opModeAlways: "Всегда на экране (Always On)",
    opModeHold: "Зажать горячую клавишу (Hold Hotkey)",
    activationHotkey: "Клавиша активации",
    hotkeyPressPrompt: "Нажмите клавишу или клик...",
    hotkeyRecorded: "Записано: {name}!",
    hotkeyCancelled: "Отменено",
    hotkeyRecordBtn: "Записать",
    autoSubmit: "Авто-отправка (Enter)",
    autoSubmitDisabled: "Выключено",
    autoSubmitEnabled: "Нажать Enter после вставки",
    leftRightMouseWarning: "Левый и правый клики мыши защищены во избежание блокировки интерфейса.",
    btnSelected: "Выбрано",
    btnUse: "Использовать",
    btnDownload: "Загрузить",
    btnCancel: "Отмена",
    btnDeleteTooltip: "Удалить модель с диска",

    // Model descriptions
    modelDescTiny: "Самый быстрый вариант для коротких заметок и быстрых команд.",
    modelDescBase: "Хороший стандартный вариант для легкого локального распознавания.",
    modelDescSmall: "Более высокая точность, при этом подходит для большинства ноутбуков.",
    modelDescMedium: "Высокое качество для длинной диктовки и смешанной речи.",
    modelDescLargeV3Turbo: "Отличное качество и более высокая скорость по сравнению с полной версией large.",
    modelDescLargeV3: "Наилучшее качество распознавания, требует больше дискового пространства и ресурсов.",

    msgCopied: "Скопировано.",
    msgTranscriptReady: "Распознано.",
    msgRecordDidNotStart: "Запись не началась.",
    msgChooseModel: "Выберите локальную модель Whisper в настройках.",
    msgEngineMissing: "Отсутствует движок Whisper. Добавьте binaries/whisper-cli.exe.",
    msgRecordingFailed: "Ошибка записи.",
    hotkeyUnassigned: "Не назначена",
  },
  uk: {
    settingsTitle: "Налаштування",
    settingsSubtitle: "Параметри моделі та розпізнавання",
    localModelSec: "Локальна модель",
    modelPath: "Активна модель",
    whisperEngine: "Движок Whisper",
    changeModelBtn: "Змінити модель",
    openFolderBtn: "Папка з моделями",
    downloadSec: "Завантажити моделі Whisper",
    selectLanguage: "Мова розпізнавання",
    qualityProfile: "Профіль якості",
    computeBackend: "Обчислювальний бекенд",
    langAuto: "Авто",
    langEn: "Англійська",
    langRu: "Російська",
    langUk: "Українська",
    langZh: "Китайська",
    langEs: "Іспанська",
    qualityFast: "Швидкий",
    qualityBalanced: "Збалансований",
    qualityAccurate: "Точний",
    hintSelectModel: "Оберіть модель у налаштуваннях",
    hintEngineMissing: "Движок відсутній",
    hintCopied: "Скопійовано.",
    hintCheckSettings: "Перевірте налаштування.",
    hintReady: "Ferrofluid Voice",
    tooltipCopy: "Скопіювати текст",
    tooltipSettings: "Наставити",
    tooltipClose: "Закрити",
    exitTitle: "Закрити додаток?",
    exitConfirm: "Ви дійсно хочете закрити додаток?",
    exitCancel: "Скасувати",
    exitSubmit: "Вийти",
    modelFound: "Модель знайдено",
    modelMissing: "Модель відсутня",
    loadingModels: "Завантаження моделей...",
    loading: "Завантаження...",
    themeDark: "Темна",
    themeLight: "Світла",

    // Settings translations
    tabModels: "Керування моделями",
    tabHotkeys: "Гарячі клавіші",
    tabGeneral: "Основні налаштування",
    hotkeySec: "Гарячі клавіші та автоматизація",
    opMode: "Режим роботи",
    opModeAlways: "Завжди на екрані (Always On)",
    opModeHold: "Затиснути гарячу клавішу (Hold Hotkey)",
    activationHotkey: "Клавіша активації",
    hotkeyPressPrompt: "Натисніть клавішу або клік...",
    hotkeyRecorded: "Записано: {name}!",
    hotkeyCancelled: "Скасовано",
    hotkeyRecordBtn: "Записати",
    autoSubmit: "Авто-надсилання (Enter)",
    autoSubmitDisabled: "Вимкнено",
    autoSubmitEnabled: "Натиснути Enter після вставки",
    leftRightMouseWarning: "Лівий та правий кліки миші захищені для уникнення блокування інтерфейсу.",
    btnSelected: "Обрано",
    btnUse: "Використати",
    btnDownload: "Завантажити",
    btnCancel: "Скасувати",
    btnDeleteTooltip: "Вилити модель з диска",

    // Model descriptions
    modelDescTiny: "Найшвидший варіант для коротких нотаток та швидких команд.",
    modelDescBase: "Гарний стандартний варіант для легкого локального розпізнавання.",
    modelDescSmall: "Вища точність, підходить для більшості ноутбуків.",
    modelDescMedium: "Висока якість для тривалого диктування та змішаного мовлення.",
    modelDescLargeV3Turbo: "Чудова якість та вища швидкість порівняно з повною версією large.",
    modelDescLargeV3: "Найкраща якість розпізнавання, потребує більше дискового простору та ресурсів.",

    msgCopied: "Скопійовано.",
    msgTranscriptReady: "Розпізнано.",
    msgRecordDidNotStart: "Запис не розпочався.",
    msgChooseModel: "Оберіть локальну модель Whisper у налаштуваннях.",
    msgEngineMissing: "Движок Whisper відсутній. Додайте binaries/whisper-cli.exe.",
    msgRecordingFailed: "Помилка запису.",
    hotkeyUnassigned: "Не призначена",
  },
  zh: {
    settingsTitle: "设置",
    settingsSubtitle: "模型与语音转文字首选项",
    localModelSec: "本地模型",
    modelPath: "活动模型",
    whisperEngine: "Whisper 引擎",
    changeModelBtn: "更换模型",
    openFolderBtn: "打开模型文件夹",
    downloadSec: "下载 Whisper 模型",
    selectLanguage: "识别语言",
    qualityProfile: "质量配置",
    computeBackend: "计算后端",
    langAuto: "自动",
    langEn: "英语",
    langRu: "俄语",
    langUk: "乌克兰语",
    langZh: "中文",
    langEs: "西班牙语",
    qualityFast: "快速",
    qualityBalanced: "均衡",
    qualityAccurate: "精确",
    hintSelectModel: "在设置中选择模型",
    hintEngineMissing: "引擎缺失",
    hintCopied: "已复制",
    hintCheckSettings: "检查设置",
    hintReady: "Ferrofluid Voice",
    tooltipCopy: "复制文本",
    tooltipSettings: "设置",
    tooltipClose: "关闭",
    exitTitle: "退出应用程序？",
    exitConfirm: "您确定要关闭该应用程序吗？",
    exitCancel: "取消",
    exitSubmit: "退出",
    modelFound: "模型已找到",
    modelMissing: "模型缺失",
    loadingModels: "正在加载模型...",
    loading: "正在加载...",
    themeDark: "暗黑",
    themeLight: "高亮",

    // Settings translations
    tabModels: "模型管理",
    tabHotkeys: "快捷键设置",
    tabGeneral: "常规设置",
    hotkeySec: "快捷键与自动化",
    opMode: "工作模式",
    opModeAlways: "总是显示 (Always On)",
    opModeHold: "按住快捷键 (Hold Hotkey)",
    activationHotkey: "激活快捷键",
    hotkeyPressPrompt: "请按任意键或点击...",
    hotkeyRecorded: "已录制: {name}!",
    hotkeyCancelled: "已取消",
    hotkeyRecordBtn: "录制",
    autoSubmit: "自动发送 (Enter)",
    autoSubmitDisabled: "已禁用",
    autoSubmitEnabled: "插入后自动发送",
    leftRightMouseWarning: "左键和右键已受保护以防界面锁定。",
    btnSelected: "已选择",
    btnUse: "使用",
    btnDownload: "下载",
    btnCancel: "取消",
    btnDeleteTooltip: "从磁盘删除模型",

    // Model descriptions
    modelDescTiny: "最适合简短笔记和快速命令的超快选项。",
    modelDescBase: "适合轻量本地转文字的推荐默认选项。",
    modelDescSmall: "准确度更高，在大多数笔记本电脑上依然流畅。",
    modelDescMedium: "针对较长听写和混合语音的高质量选项。",
    modelDescLargeV3Turbo: "高识别质量，比完整大模型速度更快。",
    modelDescLargeV3: "识别质量最佳的选项，需要更多的磁盘空间和计算资源。",

    msgCopied: "已复制。",
    msgTranscriptReady: "文本已就绪。",
    msgRecordDidNotStart: "录音未开始。",
    msgChooseModel: "请在设置中选择本地 Whisper 模型。",
    msgEngineMissing: "缺少 Whisper 引擎。请在 binaries/ 目录中添加 whisper-cli.exe。",
    msgRecordingFailed: "录音失败。",
    hotkeyUnassigned: "未分配",
  },
  es: {
    settingsTitle: "Ajustes",
    settingsSubtitle: "Preferencias de modelo y transcripción",
    localModelSec: "Modelo local",
    modelPath: "Modelo activo",
    whisperEngine: "Motor Whisper",
    changeModelBtn: "Cambiar modelo",
    openFolderBtn: "Abrir carpeta de modelos",
    downloadSec: "Descargar modelos Whisper",
    selectLanguage: "Idioma de reconocimiento",
    qualityProfile: "Perfil de calidad",
    computeBackend: "Backend de cómputo",
    langAuto: "Auto",
    langEn: "Inglés",
    langRu: "Ruso",
    langUk: "Ucraniano",
    langZh: "Chino",
    langEs: "Español",
    qualityFast: "Rápido",
    qualityBalanced: "Equilibrado",
    qualityAccurate: "Preciso",
    hintSelectModel: "Seleccione modelo en ajustes",
    hintEngineMissing: "Motor ausente",
    hintCopied: "Copiado.",
    hintCheckSettings: "Verifique ajustes.",
    hintReady: "Ferrofluid Voice",
    tooltipCopy: "Copiar texto",
    tooltipSettings: "Ajustes",
    tooltipClose: "Cerrar",
    exitTitle: "¿Cerrar aplicación?",
    exitConfirm: "¿Está seguro de que desea cerrar la aplicación?",
    exitCancel: "Cancelar",
    exitSubmit: "Salir",
    modelFound: "Modelo encontrado",
    modelMissing: "Modelo ausente",
    loadingModels: "Cargando modelos...",
    loading: "Cargando...",
    themeDark: "Oscuro",
    themeLight: "Claro",

    // Settings translations
    tabModels: "Gestión de Modelos",
    tabHotkeys: "Atajos y Dictado",
    tabGeneral: "Ajustes Generales",
    hotkeySec: "Atajos y Automatización",
    opMode: "Modo de Operación",
    opModeAlways: "Siempre en Pantalla (Always On)",
    opModeHold: "Mantener Atajo para Grabar",
    activationHotkey: "Atajo de Activación",
    hotkeyPressPrompt: "Presione tecla o haga clic...",
    hotkeyRecorded: "¡Grabado: {name}!",
    hotkeyCancelled: "Cancelado",
    hotkeyRecordBtn: "Grabar",
    autoSubmit: "Envío Automático (Enter)",
    autoSubmitDisabled: "Desactivado",
    autoSubmitEnabled: "Simular Enter tras insertar",
    leftRightMouseWarning: "Los clics izquierdo y derecho están protegidos para evitar bloquear la interfaz.",
    btnSelected: "Seleccionado",
    btnUse: "Usar",
    btnDownload: "Descargar",
    btnCancel: "Cancelar",
    btnDeleteTooltip: "Eliminar modelo del disco",

    // Model descriptions
    modelDescTiny: "La opción más rápida para notas cortas y comandos rápidos.",
    modelDescBase: "Buen valor predeterminado para transcripción local ligera.",
    modelDescSmall: "Mayor precisión siendo práctico en la mayoría de portátiles.",
    modelDescMedium: "Mayor calidad para dictados largos y discursos mixtos.",
    modelDescLargeV3Turbo: "Gran calidad con mejor velocidad que el modelo grande completo.",
    modelDescLargeV3: "La mejor calidad, requiere más espacio de disco y capacidad de cómputo.",

    msgCopied: "Copiado.",
    msgTranscriptReady: "Transcripción lista.",
    msgRecordDidNotStart: "La grabación no comenzó.",
    msgChooseModel: "Elija un modelo Whisper local en ajustes.",
    msgEngineMissing: "Falta el motor Whisper. Añada whisper-cli.exe en binaries.",
    msgRecordingFailed: "Grabación fallida.",
    hotkeyUnassigned: "No asignado",
  },
};

export function getAppLanguage(pref: Language): "en" | "ru" | "uk" | "zh" | "es" {
  if (pref === "ru") return "ru";
  if (pref === "uk") return "uk";
  if (pref === "zh") return "zh";
  if (pref === "es") return "es";
  if (pref === "en") return "en";

  // "auto" - read browser/system locale
  if (typeof navigator !== "undefined") {
    const locale = navigator.language.toLowerCase();
    if (locale.startsWith("ru")) return "ru";
    if (locale.startsWith("uk") || locale.startsWith("ua")) return "uk";
    if (locale.startsWith("zh")) return "zh";
    if (locale.startsWith("es")) return "es";
  }
  return "en";
}

export function useLocales(pref: Language): Translations {
  return translations[getAppLanguage(pref)];
}
