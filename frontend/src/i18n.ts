const translations: Record<string, Record<string, string>> = {
  // Chat
  'chat.title': { en: 'Chat', 'zh-CN': '聊天', 'zh-TW': '聊天', ja: 'チャット' },
  'chat.search': { en: 'Search conversations...', 'zh-CN': '搜索对话...', 'zh-TW': '搜尋對話...', ja: '会話を検索...' },
  'chat.new': { en: 'New conversation', 'zh-CN': '新对话', 'zh-TW': '新對話', ja: '新規会話' },
  'chat.recent': { en: 'RECENT', 'zh-CN': '最近', 'zh-TW': '最近', ja: '最近' },
  'chat.pinned': { en: 'PINNED', 'zh-CN': '置顶', 'zh-TW': '置頂', ja: 'ピン留め' },
  'chat.pin': { en: 'Pin', 'zh-CN': '固定', 'zh-TW': '固定', ja: 'ピン留め' },
  'chat.unpin': { en: 'Unpin', 'zh-CN': '取消固定', 'zh-TW': '取消固定', ja: 'ピン解除' },
  'chat.rename': { en: 'Rename session', 'zh-CN': '重命名会话', 'zh-TW': '重新命名', ja: '名前変更' },
  'chat.delete': { en: 'Delete session', 'zh-CN': '删除会话', 'zh-TW': '刪除', ja: '削除' },
  'chat.disconnected': { en: 'Disconnected', 'zh-CN': '已断开', 'zh-TW': '已斷開', ja: '切断' },
  'chat.connected': { en: 'Connected', 'zh-CN': '已连接', 'zh-TW': '已連接', ja: '接続済み' },
  'chat.streaming': { en: 'Streaming', 'zh-CN': '流式输出中', 'zh-TW': '串流中', ja: 'ストリーミング中' },
  'chat.streamingOther': { en: 'Streaming from other platform', 'zh-CN': '其他平台正在输出', 'zh-TW': '其他平台正在輸出', ja: '他のプラットフォームからストリーミング中' },
  'chat.sending': { en: 'Sending...', 'zh-CN': '发送中...', 'zh-TW': '發送中...', ja: '送信中...' },
  'chat.loadHistory': { en: 'Load older messages...', 'zh-CN': '加载更早消息...', 'zh-TW': '載入更早訊息...', ja: '古いメッセージを読み込む...' },
  'chat.loadingHistory': { en: 'Loading...', 'zh-CN': '加载中...', 'zh-TW': '載入中...', ja: '読み込み中...' },
  'chat.inputPlaceholder': { en: 'Message Hermes Agent...', 'zh-CN': '给 Hermes Agent 发消息...', 'zh-TW': '給 Hermes Agent 發訊息...', ja: 'Hermes Agentにメッセージ...' },
  'chat.draftTitle': { en: 'Draft', 'zh-CN': '草稿', 'zh-TW': '草稿', ja: '下書き' },
  'chat.you': { en: 'You', 'zh-CN': '你', 'zh-TW': '你', ja: 'あなた' },
  'chat.system': { en: 'System', 'zh-CN': '系统', 'zh-TW': '系統', ja: 'システム' },
  'chat.tool': { en: 'Tool', 'zh-CN': '工具', 'zh-TW': '工具', ja: 'ツール' },
  'chat.hermesAgent': { en: 'Hermes Agent', 'zh-CN': 'Hermes Agent', 'zh-TW': 'Hermes Agent', ja: 'Hermes Agent' },
  'chat.selectModel': { en: 'Select model', 'zh-CN': '选择模型', 'zh-TW': '選擇模型', ja: 'モデル選択' },
  'chat.searchModels': { en: 'Search models...', 'zh-CN': '搜索模型...', 'zh-TW': '搜尋模型...', ja: 'モデルを検索...' },
  'chat.noModels': { en: 'No models', 'zh-CN': '无模型', 'zh-TW': '無模型', ja: 'モデルなし' },
  'chat.attachFiles': { en: 'Attach files', 'zh-CN': '添加附件', 'zh-TW': '添加附件', ja: 'ファイル添付' },
  'chat.showThinking': { en: 'Show reasoning / thinking', 'zh-CN': '显示 reasoning / thinking', 'zh-TW': '顯示 reasoning / thinking', ja: 'reasoning / thinkingを表示' },
  'chat.hideThinking': { en: 'Hide reasoning / thinking', 'zh-CN': '隐藏 reasoning / thinking', 'zh-TW': '隱藏 reasoning / thinking', ja: 'reasoning / thinkingを非表示' },
  'chat.send': { en: 'Send', 'zh-CN': '发送', 'zh-TW': '發送', ja: '送信' },
  'chat.renameTitle': { en: 'Rename session', 'zh-CN': '重命名', 'zh-TW': '重新命名', ja: 'セッション名変更' },
  'chat.deleteTitle': { en: 'Delete session', 'zh-CN': '删除会话', 'zh-TW': '刪除會話', ja: 'セッション削除' },
  'chat.deleteConfirm': { en: 'Delete this session? This cannot be undone.', 'zh-CN': '确定删除此会话？此操作不可撤销。', 'zh-TW': '確定刪除此會話？此操作不可撤銷。', ja: 'このセッションを削除しますか？元に戻せません。' },

  // Cron
  'cron.title': { en: 'Cron', 'zh-CN': '定时任务', 'zh-TW': '定時任務', ja: 'Cron' },
  'cron.jobs': { en: 'Cron jobs', 'zh-CN': '定时任务', 'zh-TW': '定時任務', ja: 'cronジョブ' },
  'cron.scheduled': { en: 'scheduled jobs', 'zh-CN': '个已调度任务', 'zh-TW': '個已排程任務', ja: '件のスケジュール済みジョブ' },
  'cron.newJob': { en: 'New job', 'zh-CN': '新建任务', 'zh-TW': '新增任務', ja: '新規ジョブ' },
  'cron.newCron': { en: 'New cron job', 'zh-CN': '新建定时任务', 'zh-TW': '新增定時任務', ja: '新規cronジョブ' },
  'cron.editCron': { en: 'Edit cron job', 'zh-CN': '编辑定时任务', 'zh-TW': '編輯定時任務', ja: 'cronジョブ編集' },
  'cron.name': { en: 'Name', 'zh-CN': '名称', 'zh-TW': '名稱', ja: '名前' },
  'cron.schedule': { en: 'Schedule', 'zh-CN': '调度', 'zh-TW': '排程', ja: 'スケジュール' },
  'cron.prompt': { en: 'PROMPT', 'zh-CN': '提示词', 'zh-TW': '提示詞', ja: 'プロンプト' },
  'cron.script': { en: 'SCRIPT', 'zh-CN': '脚本', 'zh-TW': '腳本', ja: 'スクリプト' },
  'cron.deliver': { en: 'Delivery target', 'zh-CN': '发送渠道', 'zh-TW': '發送渠道', ja: '配信先' },
  'cron.save': { en: 'Save', 'zh-CN': '保存', 'zh-TW': '儲存', ja: '保存' },
  'cron.run': { en: 'Run now', 'zh-CN': '立即运行', 'zh-TW': '立即執行', ja: '今すぐ実行' },
  'cron.delete': { en: 'Delete', 'zh-CN': '删除', 'zh-TW': '刪除', ja: '削除' },
  'cron.saving': { en: 'Saving...', 'zh-CN': '保存中...', 'zh-TW': '儲存中...', ja: '保存中...' },
  'cron.running': { en: 'Running...', 'zh-CN': '运行中...', 'zh-TW': '執行中...', ja: '実行中...' },
  'cron.deleting': { en: 'Deleting...', 'zh-CN': '删除中...', 'zh-TW': '刪除中...', ja: '削除中...' },
  'cron.active': { en: 'active', 'zh-CN': '活跃', 'zh-TW': '活躍', ja: 'アクティブ' },
  'cron.paused': { en: 'paused', 'zh-CN': '暂停', 'zh-TW': '暫停', ja: '停止中' },
  'cron.saved': { en: 'Job saved', 'zh-CN': '任务已保存', 'zh-TW': '任務已儲存', ja: 'ジョブを保存しました' },
  'cron.deleted': { en: 'Job deleted', 'zh-CN': '任务已删除', 'zh-TW': '任務已刪除', ja: 'ジョブを削除しました' },
  'cron.ran': { en: 'Job triggered', 'zh-CN': '任务已触发', 'zh-TW': '任務已觸發', ja: 'ジョブを実行しました' },
  'cron.placeholderName': { en: 'Job name', 'zh-CN': '任务名称', 'zh-TW': '任務名稱', ja: 'ジョブ名' },
  'cron.placeholderSchedule': { en: 'Schedule, e.g. 0 9 * * *', 'zh-CN': '调度，如 0 9 * * *', 'zh-TW': '排程，如 0 9 * * *', ja: 'スケジュール 例: 0 9 * * *' },
  'cron.placeholderPrompt': { en: 'Prompt', 'zh-CN': '提示词', 'zh-TW': '提示詞', ja: 'プロンプト' },
  'cron.placeholderScript': { en: 'Script (optional)', 'zh-CN': '脚本（可选）', 'zh-TW': '腳本（可選）', ja: 'スクリプト（任意）' },

  // Memory
  'memory.title': { en: 'Memory manager', 'zh-CN': '记忆管理器', 'zh-TW': '記憶管理器', ja: 'メモリ管理' },
  'memory.subtitle': { en: 'Local Hermes memory files', 'zh-CN': '本地 Hermes 记忆文件', 'zh-TW': '本地 Hermes 記憶檔案', ja: 'ローカルHermesメモリファイル' },
  'memory.save': { en: 'Save memory files', 'zh-CN': '保存记忆文件', 'zh-TW': '儲存記憶檔案', ja: 'メモリファイルを保存' },
  'memory.saved': { en: 'Memory saved', 'zh-CN': '记忆已保存', 'zh-TW': '記憶已儲存', ja: 'メモリを保存しました' },

  // Images / Gallery
  'gallery.title': { en: 'Gallery', 'zh-CN': '图库', 'zh-TW': '圖庫', ja: 'ギャラリー' },
  'gallery.loaded': { en: 'images', 'zh-CN': '张图片', 'zh-TW': '張圖片', ja: '枚の画像' },
  'gallery.select': { en: 'Select', 'zh-CN': '选择', 'zh-TW': '選擇', ja: '選択' },
  'gallery.cancel': { en: 'Cancel', 'zh-CN': '取消', 'zh-TW': '取消', ja: 'キャンセル' },
  'gallery.selectImages': { en: 'Select images', 'zh-CN': '选择图片', 'zh-TW': '選擇圖片', ja: '画像を選択' },
  'gallery.cancelSelection': { en: 'Cancel selection', 'zh-CN': '取消选择', 'zh-TW': '取消選擇', ja: '選択解除' },
  'gallery.refresh': { en: 'Refresh', 'zh-CN': '刷新', 'zh-TW': '重新整理', ja: '更新' },
  'gallery.download': { en: 'Download', 'zh-CN': '下载', 'zh-TW': '下載', ja: 'ダウンロード' },
  'gallery.downloadHEIC': { en: 'Download HEIC', 'zh-CN': '下载 HEIC', 'zh-TW': '下載 HEIC', ja: 'HEICダウンロード' },
  'gallery.downloadPNG': { en: 'Download PNG', 'zh-CN': '下载 PNG', 'zh-TW': '下載 PNG', ja: 'PNGダウンロード' },
  'gallery.downloadSelected': { en: 'Download selected', 'zh-CN': '下载选中', 'zh-TW': '下載選取', ja: '選択をダウンロード' },
  'gallery.generateHeic': { en: 'Generate HEIC', 'zh-CN': '生成 HEIC', 'zh-TW': '生成 HEIC', ja: 'HEIC生成' },
  'gallery.organize': { en: 'Organize', 'zh-CN': '整理时间', 'zh-TW': '整理時間', ja: '整理' },
  'gallery.deleteSelected': { en: 'Delete selected', 'zh-CN': '删除选中', 'zh-TW': '刪除選取', ja: '選択を削除' },
  'gallery.previous': { en: 'Previous', 'zh-CN': '上一张', 'zh-TW': '上一張', ja: '前へ' },
  'gallery.next': { en: 'Next', 'zh-CN': '下一张', 'zh-TW': '下一張', ja: '次へ' },
  'gallery.close': { en: 'Close', 'zh-CN': '关闭', 'zh-TW': '關閉', ja: '閉じる' },
  'gallery.metadata': { en: 'Metadata', 'zh-CN': '元数据', 'zh-TW': '元資料', ja: 'メタデータ' },
  'gallery.dimensions': { en: 'Dimensions', 'zh-CN': '尺寸', 'zh-TW': '尺寸', ja: 'サイズ' },
  'gallery.files': { en: 'Files', 'zh-CN': '文件', 'zh-TW': '檔案', ja: 'ファイル' },
  'gallery.pngMetadata': { en: 'PNG metadata', 'zh-CN': 'PNG 元数据', 'zh-TW': 'PNG 元資料', ja: 'PNGメタデータ' },
  'gallery.noPngText': { en: 'No PNG text chunk', 'zh-CN': '无 PNG text chunk', 'zh-TW': '無 PNG text chunk', ja: 'PNGテキストチャンクなし' },
  'gallery.noImages': { en: 'No images found', 'zh-CN': '未找到图片', 'zh-TW': '未找到圖片', ja: '画像が見つかりません' },
  'gallery.noImagesDesc': { en: 'The binary reads the configured image directory; default is HERMES_HOME/cache/images.', 'zh-CN': '二进制读取配置的图像目录；默认 HERMES_HOME/cache/images。', 'zh-TW': '二進位讀取設定的圖像目錄；預設 HERMES_HOME/cache/images。', ja: 'バイナリは設定された画像ディレクトリを読み取ります。デフォルトは HERMES_HOME/cache/images です。' },
  'gallery.scrollMore': { en: 'Scroll to load more...', 'zh-CN': '滚动加载更多...', 'zh-TW': '捲動載入更多...', ja: 'スクロールでさらに読み込む...' },
  'gallery.end': { en: 'End of images', 'zh-CN': '已到底', 'zh-TW': '已到底', ja: '画像の最後です' },
  'gallery.refreshing': { en: 'Refreshing...', 'zh-CN': '刷新中...', 'zh-TW': '重新整理中...', ja: '更新中...' },
  'gallery.refreshed': { en: 'Refresh complete: added', 'zh-CN': '刷新完成：新增', 'zh-TW': '重新整理完成：新增', ja: '更新完了: 追加' },
  'gallery.refreshedUpdated': { en: 'updated', 'zh-CN': '更新', 'zh-TW': '更新', ja: '更新' },
  'gallery.refreshedNone': { en: 'Refresh complete: no new images', 'zh-CN': '刷新完成：没有新图', 'zh-TW': '重新整理完成：沒有新圖', ja: '更新完了: 新しい画像はありません' },
  'gallery.generatingHeic': { en: 'Generating HEIC for', 'zh-CN': '正在为', 'zh-TW': '正在為', ja: 'HEIC生成中:' },
  'gallery.heicDone': { en: 'HEIC generated.', 'zh-CN': 'HEIC 已生成。', 'zh-TW': 'HEIC 已生成。', ja: 'HEIC生成完了。' },
  'gallery.imagesUnavailable': { en: 'Image API unavailable', 'zh-CN': '图片 API 不可用', 'zh-TW': '圖片 API 不可用', ja: '画像APIが利用できません' },
  'gallery.refreshFailed': { en: 'Refresh failed', 'zh-CN': '刷新失败', 'zh-TW': '重新整理失敗', ja: '更新に失敗しました' },
  'gallery.deleteConfirm': { en: 'Delete {0} images?', 'zh-CN': '删除 {0} 张图片？', 'zh-TW': '刪除 {0} 張圖片？', ja: '{0}枚の画像を削除しますか？' },

  // Workspace
  'workspace.title': { en: 'Workspace', 'zh-CN': '工作区', 'zh-TW': '工作區', ja: 'ワークスペース' },
  'workspace.fileTree': { en: 'File tree', 'zh-CN': '文件树', 'zh-TW': '檔案樹', ja: 'ファイルツリー' },
  'workspace.editor': { en: 'Editor / preview', 'zh-CN': '编辑器 / 预览', 'zh-TW': '編輯器 / 預覽', ja: 'エディタ / プレビュー' },
  'workspace.selectFile': { en: 'Select a file', 'zh-CN': '选择一个文件', 'zh-TW': '選擇一個檔案', ja: 'ファイルを選択' },
  'workspace.selectFileDesc': { en: 'Folders expand in the left tree. Files open here.', 'zh-CN': '文件夹在左侧树中展开。文件在这里打开。', 'zh-TW': '資料夾在左側樹中展開。檔案在這裡打開。', ja: 'フォルダは左のツリーで展開。ファイルはここで開きます。' },
  'workspace.expand': { en: 'Expand workspace', 'zh-CN': '展开工作区', 'zh-TW': '展開工作區', ja: 'ワークスペースを展開' },
  'workspace.collapse': { en: 'Collapse workspace', 'zh-CN': '折叠工作区', 'zh-TW': '摺疊工作區', ja: 'ワークスペースを折りたたむ' },
  'workspace.openPage': { en: 'Open workspace page', 'zh-CN': '打开工作区页面', 'zh-TW': '開啟工作區頁面', ja: 'ワークスペースページを開く' },
  'workspace.download': { en: 'download', 'zh-CN': '下载', 'zh-TW': '下載', ja: 'ダウンロード' },
  'workspace.expandFolder': { en: 'expand folder', 'zh-CN': '展开文件夹', 'zh-TW': '展開資料夾', ja: 'フォルダを展開' },
  'workspace.renameItem': { en: 'Rename item', 'zh-CN': '重命名', 'zh-TW': '重新命名', ja: '名前変更' },
  'workspace.deleteItem': { en: 'Delete item', 'zh-CN': '删除', 'zh-TW': '刪除', ja: '削除' },

  // Skills
  'skills.title': { en: 'Skills', 'zh-CN': '技能', 'zh-TW': '技能', ja: 'スキル' },
  'skills.installed': { en: 'installed skills', 'zh-CN': '个已安装技能', 'zh-TW': '個已安裝技能', ja: 'インストール済みスキル' },
  'skills.search': { en: 'Search skills...', 'zh-CN': '搜索技能...', 'zh-TW': '搜尋技能...', ja: 'スキルを検索...' },
  'skills.skillFiles': { en: 'Skill files', 'zh-CN': '技能文件', 'zh-TW': '技能檔案', ja: 'スキルファイル' },
  'skills.select': { en: 'Select a skill', 'zh-CN': '选择一个技能', 'zh-TW': '選擇一個技能', ja: 'スキルを選択' },
  'skills.noDescription': { en: 'No description', 'zh-CN': '无描述', 'zh-TW': '無描述', ja: '説明なし' },
  'skills.enabled': { en: 'Enabled skill', 'zh-CN': '已启用技能', 'zh-TW': '已啟用技能', ja: 'スキルを有効化' },
  'skills.disabled': { en: 'Disabled skill', 'zh-CN': '已禁用技能', 'zh-TW': '已停用技能', ja: 'スキルを無効化' },

  // Settings
  'settings.title': { en: 'Settings', 'zh-CN': '设置', 'zh-TW': '設定', ja: '設定' },
  'settings.apiBase': { en: 'Hermes API base', 'zh-CN': 'Hermes API 地址', 'zh-TW': 'Hermes API 位址', ja: 'Hermes APIベース' },
  'settings.apiKey': { en: 'External API key', 'zh-CN': '外部 API key', 'zh-TW': '外部 API key', ja: '外部APIキー' },
  'settings.language': { en: 'Language', 'zh-CN': '语言', 'zh-TW': '語言', ja: '言語' },
  'settings.theme': { en: 'Theme', 'zh-CN': '主题', 'zh-TW': '主題', ja: 'テーマ' },
  'settings.followUpBehaviour': { en: 'Follow-up behaviour', 'zh-CN': '追问行为', 'zh-TW': '追問行為', ja: 'フォローアップ動作' },
  'settings.refreshConn': { en: 'Refresh connection', 'zh-CN': '刷新连接', 'zh-TW': '重新整理連線', ja: '接続を更新' },

  // Navigation
  'nav.chat': { en: 'Chat', 'zh-CN': '聊天', 'zh-TW': '聊天', ja: 'チャット' },
  'nav.cron': { en: 'Cron', 'zh-CN': '定时', 'zh-TW': '定時', ja: 'Cron' },
  'nav.memory': { en: 'Memory', 'zh-CN': '记忆', 'zh-TW': '記憶', ja: 'メモリ' },
  'nav.insights': { en: 'Insights', 'zh-CN': '洞察', 'zh-TW': '洞察', ja: 'インサイト' },
  'nav.images': { en: 'Images', 'zh-CN': '图片', 'zh-TW': '圖片', ja: '画像' },
  'nav.workspace': { en: 'Workspace', 'zh-CN': '工作区', 'zh-TW': '工作區', ja: 'ワークスペース' },
  'nav.skills': { en: 'Skills', 'zh-CN': '技能', 'zh-TW': '技能', ja: 'スキル' },
  'nav.settings': { en: 'Settings', 'zh-CN': '设置', 'zh-TW': '設定', ja: '設定' },
  'nav.openList': { en: 'Open list', 'zh-CN': '打开列表', 'zh-TW': '開啟列表', ja: 'リストを開く' },
  'nav.collapseSidebar': { en: 'Collapse sidebar', 'zh-CN': '折叠侧栏', 'zh-TW': '摺疊側欄', ja: 'サイドバーを折りたたむ' },
  'nav.expandSidebar': { en: 'Expand sidebar', 'zh-CN': '展开侧栏', 'zh-TW': '展開側欄', ja: 'サイドバーを展開' },
  'nav.mobile': { en: 'Mobile navigation', 'zh-CN': '移动端导航', 'zh-TW': '行動端導覽', ja: 'モバイルナビゲーション' },
  'nav.theme': { en: 'Theme', 'zh-CN': '主题', 'zh-TW': '主題', ja: 'テーマ' },

  // Tool messages
  'tool.expand': { en: 'Show details', 'zh-CN': '显示详情', 'zh-TW': '顯示詳情', ja: '詳細を表示' },
  'tool.collapse': { en: 'Hide details', 'zh-CN': '隐藏详情', 'zh-TW': '隱藏詳情', ja: '詳細を非表示' },

  // Dialogs
  'dialog.cancel': { en: 'Cancel', 'zh-CN': '取消', 'zh-TW': '取消', ja: 'キャンセル' },
  'dialog.ok': { en: 'OK', 'zh-CN': '确定', 'zh-TW': '確定', ja: 'OK' },
  'dialog.deleteImages': { en: 'Delete images', 'zh-CN': '删除图片', 'zh-TW': '刪除圖片', ja: '画像を削除' },
  'dialog.renameTitle': { en: 'Rename', 'zh-CN': '重命名', 'zh-TW': '重新命名', ja: '名前変更' },

  // Status messages
  'status.skillsLoaded': { en: 'Skills loaded', 'zh-CN': '技能已加载', 'zh-TW': '技能已載入', ja: 'スキル読み込み完了' },
  'status.modelsLoaded': { en: 'Models loaded', 'zh-CN': '模型已加载', 'zh-TW': '模型已載入', ja: 'モデル読み込み完了' },
  'status.connecting': { en: 'Connecting...', 'zh-CN': '连接中...', 'zh-TW': '連線中...', ja: '接続中...' },

  // Session time
  'time.justNow': { en: 'Just now', 'zh-CN': '刚刚', 'zh-TW': '剛剛', ja: 'たった今' },

  // Misc
  'misc.messages': { en: 'messages', 'zh-CN': '条消息', 'zh-TW': '條訊息', ja: 'メッセージ' },
};

export type Lang = 'en' | 'zh-CN' | 'zh-TW' | 'ja';

const LANG_KEY = 'lang';

let currentLang: Lang = 'en';

function detectBrowserLang(): Lang {
  try {
    const nav = (typeof navigator !== 'undefined' && navigator.language) || '';
    if (nav.startsWith('zh-TW') || nav.startsWith('zh-HK')) return 'zh-TW';
    if (nav.startsWith('zh')) return 'zh-CN';
    if (nav.startsWith('ja')) return 'ja';
    return 'en';
  } catch { return 'en'; }
}

export function initLang(): Lang {
  try {
    const stored = localStorage.getItem(LANG_KEY);
    if (stored && (stored === 'en' || stored === 'zh-CN' || stored === 'zh-TW' || stored === 'ja')) {
      currentLang = stored;
      return stored;
    }
    // First visit: detect from browser, persist once
    const detected = detectBrowserLang();
    currentLang = detected;
    try { localStorage.setItem(LANG_KEY, detected); } catch {}
    return detected;
  } catch { return currentLang; }
}

export function setLang(lang: Lang) {
  currentLang = lang;
  try { localStorage.setItem(LANG_KEY, lang); } catch {}
}

export function getLang(): Lang {
  return currentLang;
}

export function t(key: string): string {
  const entry = translations[key];
  if (!entry) return key;
  return entry[currentLang] || entry.en || key;
}

export function tf(key: string, ...args: (string | number)[]): string {
  let result = t(key);
  args.forEach((arg, i) => {
    result = result.replace(`{${i}}`, String(arg));
  });
  return result;
}
