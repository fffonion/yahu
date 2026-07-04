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
  'chat.hideCronSessions': { en: 'Hide cron conversations', 'zh-CN': '隐藏定时任务对话', 'zh-TW': '隱藏定時任務對話', ja: 'cron会話を非表示' },
  'chat.showCronSessions': { en: 'Show cron conversations', 'zh-CN': '显示定时任务对话', 'zh-TW': '顯示定時任務對話', ja: 'cron会話を表示' },
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
  'chat.showToolCalls': { en: 'Show tool calls', 'zh-CN': '显示 tool calls', 'zh-TW': '顯示 tool calls', ja: 'tool callsを表示' },
  'chat.hideToolCalls': { en: 'Hide tool calls', 'zh-CN': '隐藏 tool calls', 'zh-TW': '隱藏 tool calls', ja: 'tool callsを非表示' },
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
  'cron.pinnedModel': { en: 'Pinned model/provider', 'zh-CN': '固定模型/服务商', 'zh-TW': '固定模型/服務商', ja: '固定モデル/プロバイダー' },
  'cron.noPinnedModel': { en: 'using runtime default model', 'zh-CN': '使用运行时默认模型', 'zh-TW': '使用執行時預設模型', ja: '実行時の既定モデルを使用' },
  'cron.nonAgentJob': { en: 'non-agent task', 'zh-CN': '非Agent任务', 'zh-TW': '非Agent任務', ja: '非Agentタスク' },
  'cron.enabledTools': { en: 'Enabled tools', 'zh-CN': '启用工具', 'zh-TW': '啟用工具', ja: '有効なツール' },
  'cron.allDefaultTools': { en: 'all default tools', 'zh-CN': '默认全部工具', 'zh-TW': '預設全部工具', ja: '既定の全ツール' },
  'cron.save': { en: 'Save', 'zh-CN': '保存', 'zh-TW': '儲存', ja: '保存' },
  'cron.run': { en: 'Run now', 'zh-CN': '立即运行', 'zh-TW': '立即執行', ja: '今すぐ実行' },
  'cron.delete': { en: 'Delete', 'zh-CN': '删除', 'zh-TW': '刪除', ja: '削除' },
  'cron.deleteTitle': { en: 'Delete cron job', 'zh-CN': '删除定时任务', 'zh-TW': '刪除定時任務', ja: 'cronジョブを削除' },
  'cron.deleteConfirm': { en: 'Delete cron job “{0}”? This cannot be undone.', 'zh-CN': '确定删除定时任务“{0}”？此操作不可撤销。', 'zh-TW': '確定刪除定時任務「{0}」？此操作不可撤銷。', ja: 'cronジョブ「{0}」を削除しますか？元に戻せません。' },
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
  'cron.placeholderDeliver': { en: 'origin, local, all, telegram, or platform:chat_id', 'zh-CN': 'origin、local、all、telegram 或 platform:chat_id', 'zh-TW': 'origin、local、all、telegram 或 platform:chat_id', ja: 'origin、local、all、telegram、または platform:chat_id' },
  'cron.noSchedule': { en: 'no schedule', 'zh-CN': '无调度', 'zh-TW': '無排程', ja: 'スケジュールなし' },
  'cron.deliverOrigin': { en: 'origin (reply to chat)', 'zh-CN': 'origin（回复当前聊天）', 'zh-TW': 'origin（回覆目前聊天）', ja: 'origin（チャットへ返信）' },
  'cron.deliverLocal': { en: 'local only', 'zh-CN': '仅本地', 'zh-TW': '僅本地', ja: 'ローカルのみ' },
  'cron.deliverAll': { en: 'all connected channels', 'zh-CN': '所有已连接渠道', 'zh-TW': '所有已連接渠道', ja: '接続済み全チャンネル' },
  'cron.saveAria': { en: 'save cron job', 'zh-CN': '保存定时任务', 'zh-TW': '儲存定時任務', ja: 'cronジョブを保存' },
  'cron.runAria': { en: 'run cron job', 'zh-CN': '运行定时任务', 'zh-TW': '執行定時任務', ja: 'cronジョブを実行' },
  'cron.deleteAria': { en: 'delete cron job', 'zh-CN': '删除定时任务', 'zh-TW': '刪除定時任務', ja: 'cronジョブを削除' },
  'cron.runShort': { en: 'Run', 'zh-CN': '运行', 'zh-TW': '執行', ja: '実行' },
  'cron.jobsUnavailable': { en: 'Jobs unavailable: {0}', 'zh-CN': '任务不可用：{0}', 'zh-TW': '任務不可用：{0}', ja: 'ジョブが利用できません: {0}' },
  'cron.lastOutput': { en: 'Last output', 'zh-CN': '上次运行结果', 'zh-TW': '上次執行結果', ja: '前回の実行結果' },
  'cron.refreshOutput': { en: 'Refresh', 'zh-CN': '刷新', 'zh-TW': '重新整理', ja: '更新' },
  'cron.loadingOutput': { en: 'Loading output…', 'zh-CN': '正在加载结果…', 'zh-TW': '正在載入結果…', ja: '結果を読み込み中…' },
  'cron.noOutput': { en: 'No saved output yet.', 'zh-CN': '还没有保存的运行结果。', 'zh-TW': '尚未保存執行結果。', ja: '保存済みの実行結果はまだありません。' },
  'cron.outputUnavailable': { en: 'Output unavailable: {0}', 'zh-CN': '运行结果不可用：{0}', 'zh-TW': '執行結果不可用：{0}', ja: '実行結果を取得できません: {0}' },
  'cron.outputTruncated': { en: '[Output truncated]', 'zh-CN': '【结果已截断】', 'zh-TW': '【結果已截斷】', ja: '【結果は省略されました】' },

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
  'workspace.openFullPreview': { en: 'Open full workspace preview', 'zh-CN': '打开工作区大屏预览', 'zh-TW': '開啟工作區大屏預覽', ja: 'ワークスペースの大きなプレビューを開く' },
  'workspace.download': { en: 'download', 'zh-CN': '下载', 'zh-TW': '下載', ja: 'ダウンロード' },
  'workspace.expandFolder': { en: 'expand folder', 'zh-CN': '展开文件夹', 'zh-TW': '展開資料夾', ja: 'フォルダを展開' },
  'workspace.editItem': { en: 'Edit item', 'zh-CN': '编辑', 'zh-TW': '編輯', ja: '編集' },
  'workspace.viewItem': { en: 'View file', 'zh-CN': '查看文件', 'zh-TW': '檢視檔案', ja: 'ファイルを表示' },
  'workspace.editItemPage': { en: 'Edit file', 'zh-CN': '编辑文件', 'zh-TW': '編輯檔案', ja: 'ファイルを編集' },
  'workspace.renameItem': { en: 'Rename item', 'zh-CN': '重命名', 'zh-TW': '重新命名', ja: '名前変更' },
  'workspace.deleteItem': { en: 'Delete item', 'zh-CN': '删除', 'zh-TW': '刪除', ja: '削除' },
  'workspace.downloadFile': { en: 'Download file', 'zh-CN': '下载文件', 'zh-TW': '下載檔案', ja: 'ファイルをダウンロード' },
  'workspace.edit': { en: 'Edit', 'zh-CN': '编辑', 'zh-TW': '編輯', ja: '編集' },
  'workspace.cancelEdit': { en: 'Cancel edit', 'zh-CN': '取消编辑', 'zh-TW': '取消編輯', ja: '編集をキャンセル' },
  'workspace.closePreview': { en: 'Close preview', 'zh-CN': '关闭预览', 'zh-TW': '關閉預覽', ja: 'プレビューを閉じる' },
  'workspace.main': { en: 'MAIN', 'zh-CN': '主区', 'zh-TW': '主區', ja: 'メイン' },
  'workspace.full': { en: 'FULL', 'zh-CN': '完整', 'zh-TW': '完整', ja: '全体' },
  'workspace.itemNotEditable': { en: 'Workspace item is not editable', 'zh-CN': '此工作区项目不可编辑', 'zh-TW': '此工作區項目不可編輯', ja: 'このワークスペース項目は編集できません' },
  'workspace.previewFailed': { en: 'Preview failed: {0}', 'zh-CN': '预览失败：{0}', 'zh-TW': '預覽失敗：{0}', ja: 'プレビュー失敗: {0}' },
  'workspace.unavailable': { en: 'Workspace unavailable: {0}', 'zh-CN': '工作区不可用：{0}', 'zh-TW': '工作區不可用：{0}', ja: 'ワークスペースが利用できません: {0}' },
  'workspace.folderUnavailable': { en: 'Workspace folder unavailable: {0}', 'zh-CN': '工作区文件夹不可用：{0}', 'zh-TW': '工作區資料夾不可用：{0}', ja: 'ワークスペースフォルダが利用できません: {0}' },
  'workspace.routeUnavailable': { en: 'Workspace route unavailable: {0}', 'zh-CN': '工作区路径不可用：{0}', 'zh-TW': '工作區路徑不可用：{0}', ja: 'ワークスペースルートが利用できません: {0}' },
  'workspace.renameTitle': { en: 'Rename item', 'zh-CN': '重命名项目', 'zh-TW': '重新命名項目', ja: '項目名を変更' },
  'workspace.renameMessage': { en: 'Choose a new file or folder name.', 'zh-CN': '输入新的文件或文件夹名称。', 'zh-TW': '輸入新的檔案或資料夾名稱。', ja: '新しいファイル名またはフォルダ名を入力してください。' },
  'workspace.renameFailed': { en: 'Workspace rename failed: {0}', 'zh-CN': '工作区重命名失败：{0}', 'zh-TW': '工作區重新命名失敗：{0}', ja: 'ワークスペース名変更に失敗: {0}' },
  'workspace.renamed': { en: 'Renamed workspace item', 'zh-CN': '工作区项目已重命名', 'zh-TW': '工作區項目已重新命名', ja: 'ワークスペース項目名を変更しました' },
  'workspace.deleteTitle': { en: 'Delete workspace item', 'zh-CN': '删除工作区项目', 'zh-TW': '刪除工作區項目', ja: 'ワークスペース項目を削除' },
  'workspace.deleteConfirm': { en: 'Delete workspace {0} “{1}”?', 'zh-CN': '删除工作区 {0} “{1}”？', 'zh-TW': '刪除工作區 {0}「{1}」？', ja: 'ワークスペースの{0}「{1}」を削除しますか？' },
  'workspace.deleteFailed': { en: 'Workspace delete failed: {0}', 'zh-CN': '工作区删除失败：{0}', 'zh-TW': '工作區刪除失敗：{0}', ja: 'ワークスペース削除に失敗: {0}' },
  'workspace.deleted': { en: 'Deleted workspace item', 'zh-CN': '工作区项目已删除', 'zh-TW': '工作區項目已刪除', ja: 'ワークスペース項目を削除しました' },
  'workspace.saveFailed': { en: 'Save failed: {0}', 'zh-CN': '保存失败：{0}', 'zh-TW': '儲存失敗：{0}', ja: '保存に失敗: {0}' },

  // Insights
  'insights.title': { en: 'Insights', 'zh-CN': '洞察', 'zh-TW': '洞察', ja: 'インサイト' },
  'insights.loadingUsage': { en: 'Loading usage…', 'zh-CN': '正在加载用量…', 'zh-TW': '正在載入用量…', ja: '使用量を読み込み中…' },
  'insights.unavailable': { en: 'Usage insights unavailable', 'zh-CN': '用量洞察不可用', 'zh-TW': '用量洞察不可用', ja: '使用量インサイトは利用できません' },
  'insights.lastTokens': { en: 'Last {0} · {1} tokens', 'zh-CN': '最近 {0} · {1} tokens', 'zh-TW': '最近 {0} · {1} tokens', ja: '直近{0} · {1} tokens' },
  'insights.refreshUsage': { en: 'Refresh usage', 'zh-CN': '刷新用量', 'zh-TW': '重新整理用量', ja: '使用量を更新' },
  'insights.usageControls': { en: 'Usage controls', 'zh-CN': '用量控制', 'zh-TW': '用量控制', ja: '使用量コントロール' },
  'insights.usageMetric': { en: 'Usage metric', 'zh-CN': '用量指标', 'zh-TW': '用量指標', ja: '使用量メトリック' },
  'insights.metric.total_tokens': { en: 'Total', 'zh-CN': '总量', 'zh-TW': '總量', ja: '合計' },
  'insights.metric.input': { en: 'Input', 'zh-CN': '输入', 'zh-TW': '輸入', ja: '入力' },
  'insights.metric.output': { en: 'Output', 'zh-CN': '输出', 'zh-TW': '輸出', ja: '出力' },
  'insights.metric.cache_read': { en: 'Cache read', 'zh-CN': '缓存命中', 'zh-TW': '快取命中', ja: 'キャッシュ読取' },
  'insights.metric.reasoning': { en: 'Reasoning', 'zh-CN': '推理', 'zh-TW': '推理', ja: '推論' },
  'insights.metric.cost_usd': { en: 'Cost', 'zh-CN': '费用', 'zh-TW': '費用', ja: '費用' },
  'insights.tokens': { en: 'Tokens', 'zh-CN': 'Token', 'zh-TW': 'Token', ja: 'トークン' },
  'insights.inputOutputDetail': { en: '{0} in · {1} out', 'zh-CN': '{0} 输入 · {1} 输出', 'zh-TW': '{0} 輸入 · {1} 輸出', ja: '{0} 入力 · {1} 出力' },
  'insights.cacheHit': { en: 'Cache hit', 'zh-CN': '缓存命中', 'zh-TW': '快取命中', ja: 'キャッシュヒット' },
  'insights.cacheDetail': { en: '{0} read · {1} write', 'zh-CN': '{0} 读取 · {1} 写入', 'zh-TW': '{0} 讀取 · {1} 寫入', ja: '{0} 読取 · {1} 書込' },
  'insights.unpricedApiCalls': { en: '{0} unpriced · {1} API calls', 'zh-CN': '{0} 未计价 · {1} 次 API 调用', 'zh-TW': '{0} 未計價 · {1} 次 API 呼叫', ja: '{0} 未価格 · API呼び出し {1} 回' },
  'insights.sessionsApiCalls': { en: '{0} sessions · {1} API calls', 'zh-CN': '{0} 个会话 · {1} 次 API 调用', 'zh-TW': '{0} 個會話 · {1} 次 API 呼叫', ja: '{0} セッション · API呼び出し {1} 回' },
  'insights.topModel': { en: 'Top model', 'zh-CN': '最高模型', 'zh-TW': '最高模型', ja: 'トップモデル' },
  'insights.noUsage': { en: 'No usage', 'zh-CN': '无用量', 'zh-TW': '無用量', ja: '使用量なし' },
  'insights.byModel': { en: '{0} by model', 'zh-CN': '按模型统计 {0}', 'zh-TW': '按模型統計 {0}', ja: 'モデル別 {0}' },
  'insights.lastDistribution': { en: 'Last {0} distribution by model', 'zh-CN': '最近 {0} 按模型分布', 'zh-TW': '最近 {0} 按模型分布', ja: '直近{0}のモデル別分布' },
  'insights.recentTrend': { en: 'Recent {0} trend with cache/input/output usage', 'zh-CN': '最近 {0} 缓存/输入/输出趋势', 'zh-TW': '最近 {0} 快取/輸入/輸出趨勢', ja: '直近{0}のキャッシュ/入力/出力トレンド' },
  'insights.showStackedChart': { en: 'Show stacked chart', 'zh-CN': '显示堆叠图表', 'zh-TW': '顯示堆疊圖表', ja: '積み上げチャートを表示' },
  'insights.showUnstackedChart': { en: 'Show unstacked chart', 'zh-CN': '显示非堆叠图表', 'zh-TW': '顯示非堆疊圖表', ja: '非積み上げチャートを表示' },
  'insights.models': { en: 'Models', 'zh-CN': '模型', 'zh-TW': '模型', ja: 'モデル' },
  'insights.noWindowUsage': { en: 'No model usage in this window.', 'zh-CN': '此时间窗内没有模型用量。', 'zh-TW': '此時間窗內沒有模型用量。', ja: 'この期間のモデル使用量はありません。' },
  'insights.otherSignals': { en: 'Other signals', 'zh-CN': '其他信号', 'zh-TW': '其他訊號', ja: 'その他のシグナル' },
  'insights.reasoning': { en: 'Reasoning', 'zh-CN': '推理', 'zh-TW': '推理', ja: '推論' },
  'insights.tools': { en: 'Tools', 'zh-CN': '工具', 'zh-TW': '工具', ja: 'ツール' },
  'insights.avgSession': { en: 'Avg/session', 'zh-CN': '平均/会话', 'zh-TW': '平均/會話', ja: '平均/セッション' },
  'insights.sources': { en: 'Sources', 'zh-CN': '来源', 'zh-TW': '來源', ja: 'ソース' },
  'insights.noMetricUsage': { en: 'No model usage for this metric.', 'zh-CN': '此指标没有模型用量。', 'zh-TW': '此指標沒有模型用量。', ja: 'このメトリックのモデル使用量はありません。' },
  'insights.modelShare': { en: '{0} model share', 'zh-CN': '{0} 模型占比', 'zh-TW': '{0} 模型占比', ja: '{0} モデル比率' },
  'insights.trendChart': { en: 'Usage trend chart', 'zh-CN': '用量趋势图', 'zh-TW': '用量趨勢圖', ja: '使用量トレンドチャート' },
  'insights.loadingChart': { en: 'Loading chart', 'zh-CN': '正在加载图表', 'zh-TW': '正在載入圖表', ja: 'チャートを読み込み中' },
  'insights.loading': { en: 'Loading', 'zh-CN': '加载中', 'zh-TW': '載入中', ja: '読み込み中' },
  'insights.total': { en: 'Total', 'zh-CN': '总计', 'zh-TW': '總計', ja: '合計' },
  'insights.modelRowDetail': { en: '{0} input · {1} output · {2} cache', 'zh-CN': '{0} 输入 · {1} 输出 · {2} 缓存', 'zh-TW': '{0} 輸入 · {1} 輸出 · {2} 快取', ja: '{0} 入力 · {1} 出力 · {2} キャッシュ' },

  // Skills
  'skills.title': { en: 'Skills', 'zh-CN': '技能', 'zh-TW': '技能', ja: 'スキル' },
  'skills.installed': { en: 'installed skills', 'zh-CN': '个已安装技能', 'zh-TW': '個已安裝技能', ja: 'インストール済みスキル' },
  'skills.search': { en: 'Search skills...', 'zh-CN': '搜索技能...', 'zh-TW': '搜尋技能...', ja: 'スキルを検索...' },
  'skills.skillFiles': { en: 'Skill files', 'zh-CN': '技能文件', 'zh-TW': '技能檔案', ja: 'スキルファイル' },
  'skills.select': { en: 'Select a skill', 'zh-CN': '选择一个技能', 'zh-TW': '選擇一個技能', ja: 'スキルを選択' },
  'skills.noDescription': { en: 'No description', 'zh-CN': '无描述', 'zh-TW': '無描述', ja: '説明なし' },
'skills.selectHint': { en: 'Select a skill from the sidebar to view its files.', 'zh-CN': '从侧边栏选择一个技能查看文件。', 'zh-TW': '從側邊欄選擇一個技能查看檔案。', ja: 'サイドバーからスキルを選択してファイルを表示。' },
'skills.download': { en: 'Download skill', 'zh-CN': '下载技能', 'zh-TW': '下載技能', ja: 'スキルをダウンロード' },
'skills.enabled': { en: 'Enabled skill', 'zh-CN': '已启用技能', 'zh-TW': '已啟用技能', ja: 'スキルを有効化' },
  'skills.disabled': { en: 'Disabled skill', 'zh-CN': '已禁用技能', 'zh-TW': '已停用技能', ja: 'スキルを無効化' },
  'skills.delete': { en: 'Delete skill', 'zh-CN': '删除技能', 'zh-TW': '刪除技能', ja: 'スキルを削除' },
  'skills.deleteTitle': { en: 'Delete skill', 'zh-CN': '删除技能', 'zh-TW': '刪除技能', ja: 'スキルを削除' },
  'skills.deleteConfirm': { en: 'Delete skill {0}? This removes its directory.', 'zh-CN': '删除技能 {0}？这会移除它的目录。', 'zh-TW': '刪除技能 {0}？這會移除它的目錄。', ja: 'スキル {0} を削除しますか？ディレクトリも削除されます。' },
  'skills.deleteFailed': { en: 'Skill delete failed: {0}', 'zh-CN': '技能删除失败：{0}', 'zh-TW': '技能刪除失敗：{0}', ja: 'スキル削除に失敗しました: {0}' },
  'skills.deleted': { en: 'Skill deleted', 'zh-CN': '技能已删除', 'zh-TW': '技能已刪除', ja: 'スキルを削除しました' },
  'skills.renameFile': { en: 'Rename', 'zh-CN': '重命名', 'zh-TW': '重新命名', ja: '名前変更' },
  'skills.renameFileTitle': { en: 'Rename skill file', 'zh-CN': '重命名技能文件', 'zh-TW': '重新命名技能檔案', ja: 'スキルファイル名を変更' },
  'skills.renameFileMessage': { en: 'Enter the new name.', 'zh-CN': '输入新名称。', 'zh-TW': '輸入新名稱。', ja: '新しい名前を入力してください。' },
  'skills.renameFileFailed': { en: 'Skill file rename failed: {0}', 'zh-CN': '技能文件重命名失败：{0}', 'zh-TW': '技能檔案重新命名失敗：{0}', ja: 'スキルファイル名の変更に失敗しました: {0}' },
  'skills.renamedFile': { en: 'Skill file renamed', 'zh-CN': '技能文件已重命名', 'zh-TW': '技能檔案已重新命名', ja: 'スキルファイル名を変更しました' },
  'skills.deleteFile': { en: 'Delete', 'zh-CN': '删除', 'zh-TW': '刪除', ja: '削除' },
  'skills.deleteFileTitle': { en: 'Delete skill file', 'zh-CN': '删除技能文件', 'zh-TW': '刪除技能檔案', ja: 'スキルファイルを削除' },
  'skills.deleteFileConfirm': { en: 'Delete {0} {1}?', 'zh-CN': '删除 {0} {1}？', 'zh-TW': '刪除 {0} {1}？', ja: '{0} {1} を削除しますか？' },
  'skills.deleteFileFailed': { en: 'Skill file delete failed: {0}', 'zh-CN': '技能文件删除失败：{0}', 'zh-TW': '技能檔案刪除失敗：{0}', ja: 'スキルファイル削除に失敗しました: {0}' },
  'skills.deletedFile': { en: 'Skill file deleted', 'zh-CN': '技能文件已删除', 'zh-TW': '技能檔案已刪除', ja: 'スキルファイルを削除しました' },

  // Settings
  'settings.title': { en: 'Settings', 'zh-CN': '设置', 'zh-TW': '設定', ja: '設定' },
  'settings.apiUrl': { en: 'Hermes API URL', 'zh-CN': 'Hermes API 地址', 'zh-TW': 'Hermes API 位址', ja: 'Hermes API URL' },
  'settings.apiProxyBase': { en: 'Browser API proxy base', 'zh-CN': '浏览器 API 代理地址', 'zh-TW': '瀏覽器 API 代理位址', ja: 'ブラウザAPIプロキシベース' },
  'settings.apiKey': { en: 'External API key', 'zh-CN': '外部 API key', 'zh-TW': '外部 API key', ja: '外部APIキー' },
  'settings.language': { en: 'Language', 'zh-CN': '语言', 'zh-TW': '語言', ja: '言語' },
  'settings.theme': { en: 'Theme', 'zh-CN': '主题', 'zh-TW': '主題', ja: 'テーマ' },
  'settings.followUpBehaviour': { en: 'Follow-up behaviour', 'zh-CN': '追问行为', 'zh-TW': '追問行為', ja: 'フォローアップ動作' },
  'settings.composerEnterMode': { en: 'Composer Enter key', 'zh-CN': '输入框回车键', 'zh-TW': '輸入框 Enter 鍵', ja: '入力欄のEnterキー' },
  'settings.save': { en: 'Save settings', 'zh-CN': '保存设置', 'zh-TW': '儲存設定', ja: '設定を保存' },
  'settings.saved': { en: 'Settings saved', 'zh-CN': '设置已保存', 'zh-TW': '設定已儲存', ja: '設定を保存しました' },
  'settings.enterSend': { en: 'Enter sends; Ctrl+Enter inserts newline', 'zh-CN': 'Enter 发送；Ctrl+Enter 换行', 'zh-TW': 'Enter 傳送；Ctrl+Enter 換行', ja: 'Enterで送信、Ctrl+Enterで改行' },
  'settings.enterNewline': { en: 'Enter inserts newline; Ctrl+Enter sends', 'zh-CN': 'Enter 换行；Ctrl+Enter 发送', 'zh-TW': 'Enter 換行；Ctrl+Enter 傳送', ja: 'Enterで改行、Ctrl+Enterで送信' },
  
'settings.refreshConn': { en: 'Refresh connection', 'zh-CN': '刷新连接', 'zh-TW': '重新整理連線', ja: '接続を更新' },
'settings.update': { en: 'Update', 'zh-CN': '更新', 'zh-TW': '更新', ja: 'アップデート' },
'settings.version': { en: 'Version', 'zh-CN': '版本', 'zh-TW': '版本', ja: 'バージョン' },
'settings.checkUpdate': { en: 'Check for updates', 'zh-CN': '检查更新', 'zh-TW': '檢查更新', ja: 'アップデート確認' },
'settings.updateAvailable': { en: 'Update available', 'zh-CN': '有新版本', 'zh-TW': '有新版本', ja: 'アップデートあり' },
'settings.upToDate': { en: 'Up to date', 'zh-CN': '已是最新', 'zh-TW': '已是最新', ja: '最新です' },
'settings.viewRelease': { en: 'View release', 'zh-CN': '查看发布', 'zh-TW': '查看發布', ja: 'リリースを見る' },
'settings.installUpdate': { en: 'Install update', 'zh-CN': '安装更新', 'zh-TW': '安裝更新', ja: 'アップデートをインストール' },
'settings.checkingUpdate': { en: 'Checking…', 'zh-CN': '检查中…', 'zh-TW': '檢查中…', ja: '確認中…' },
'settings.installingUpdate': { en: 'Installing update…', 'zh-CN': '正在安装更新…', 'zh-TW': '正在安裝更新…', ja: 'アップデートをインストール中…' },
'settings.restartingUpdate': { en: 'Restarting…', 'zh-CN': '正在重启…', 'zh-TW': '正在重啟…', ja: '再起動中…' },

  // Navigation
  'nav.chat': { en: 'Chat', 'zh-CN': '聊天', 'zh-TW': '聊天', ja: 'チャット' },
  'nav.cron': { en: 'Cron', 'zh-CN': '定时', 'zh-TW': '定時', ja: 'Cron' },
  'nav.memory': { en: 'Memory', 'zh-CN': '记忆', 'zh-TW': '記憶', ja: 'メモリ' },
  'nav.insights': { en: 'Insights', 'zh-CN': '洞察', 'zh-TW': '洞察', ja: 'インサイト' },
  'nav.artifacts': { en: 'Artifacts', 'zh-CN': '看板', 'zh-TW': '看板', ja: 'Artifacts' },
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
  'dialog.save': { en: 'Save', 'zh-CN': '保存', 'zh-TW': '儲存', ja: '保存' },
  'dialog.confirm': { en: 'Confirm', 'zh-CN': '确认', 'zh-TW': '確認', ja: '確認' },

  // Theme
  'theme.appearance': { en: 'Appearance', 'zh-CN': '外观', 'zh-TW': '外觀', ja: '外観' },
  'theme.theme': { en: 'Theme', 'zh-CN': '主题', 'zh-TW': '主題', ja: 'テーマ' },

  // Mode sidebars
  'mode.cronSummary': { en: 'Create and manage scheduled jobs.', 'zh-CN': '创建并管理定时任务。', 'zh-TW': '建立並管理定時任務。', ja: 'スケジュール済みジョブを作成・管理します。' },
  'mode.memorySummary': { en: 'Edit MEMORY.md and USER.md in a full-width editor.', 'zh-CN': '在全宽编辑器中编辑 MEMORY.md 和 USER.md。', 'zh-TW': '在全寬編輯器中編輯 MEMORY.md 和 USER.md。', ja: '全幅エディタで MEMORY.md と USER.md を編集します。' },
  'mode.insightsSummary': { en: 'Recent model usage, cache, and cost trends.', 'zh-CN': '近期模型用量、缓存与费用趋势。', 'zh-TW': '近期模型用量、快取與費用趨勢。', ja: '最近のモデル使用量、キャッシュ、費用トレンド。' },
  'mode.artifactsSummary': { en: 'Session artifacts, reports, and reusable work surfaces.', 'zh-CN': '会话看板、报告和可复用工作页面。', 'zh-TW': '會話看板、報告和可複用工作頁面。', ja: 'セッション成果物、レポート、再利用できる作業ページ。' },
  'mode.workspaceSummary': { en: 'Browse and preview local workspace files.', 'zh-CN': '浏览并预览本地工作区文件。', 'zh-TW': '瀏覽並預覽本地工作區檔案。', ja: 'ローカルワークスペースファイルを閲覧・プレビューします。' },
  'mode.settingsSummary': { en: 'API, connection, and WebUI options.', 'zh-CN': 'API、连接与 WebUI 选项。', 'zh-TW': 'API、連線與 WebUI 選項。', ja: 'API、接続、WebUIオプション。' },
  'mode.imagesSummary': { en: 'Native image gallery.', 'zh-CN': '原生图片图库。', 'zh-TW': '原生圖片圖庫。', ja: 'ネイティブ画像ギャラリー。' },

  // Artifacts
  'artifacts.title': { en: 'Artifacts', 'zh-CN': '看板', 'zh-TW': '看板', ja: 'Artifacts' },
  'artifacts.subtitle': { en: 'Published session reports and work surfaces', 'zh-CN': '已发布的会话报告和工作页面', 'zh-TW': '已發布的會話報告和工作頁面', ja: '公開済みセッションレポートと作業ページ' },
  'artifacts.createFromSession': { en: 'Create artifact from session', 'zh-CN': '从会话生成看板', 'zh-TW': '從會話生成看板', ja: 'セッションからArtifactを作成' },
  'artifacts.created': { en: 'Artifact created', 'zh-CN': '看板已生成', 'zh-TW': '看板已生成', ja: 'Artifactを作成しました' },
  'artifacts.empty': { en: 'No artifacts yet', 'zh-CN': '暂无看板', 'zh-TW': '暫無看板', ja: 'Artifactはまだありません' },
  'artifacts.emptyDesc': { en: 'Open a chat session and publish it from the header.', 'zh-CN': '打开会话后可从顶部按钮生成。', 'zh-TW': '開啟會話後可從頂部按鈕生成。', ja: 'チャットセッションを開き、ヘッダーから公開します。' },
  'artifacts.copyPrompt': { en: 'Copy as prompt', 'zh-CN': '复制为提示词', 'zh-TW': '複製為提示詞', ja: 'プロンプトとしてコピー' },
  'artifacts.copiedPrompt': { en: 'Artifact prompt copied', 'zh-CN': '提示词已复制', 'zh-TW': '提示詞已複製', ja: 'Artifactプロンプトをコピーしました' },
  'artifacts.copyFailed': { en: 'Copy failed; check browser clipboard permission.', 'zh-CN': '复制失败，请检查浏览器剪贴板权限。', 'zh-TW': '複製失敗，請檢查瀏覽器剪貼簿權限。', ja: 'コピーに失敗しました。ブラウザのクリップボード権限を確認してください。' },
  'artifacts.latestVersion': { en: 'Latest version', 'zh-CN': '最新版本', 'zh-TW': '最新版本', ja: '最新バージョン' },
  'artifacts.versionHelp': { en: 'A new version is appended every time this source is published again.', 'zh-CN': '同一来源每次重新生成都会追加一个新版本。', 'zh-TW': '同一來源每次重新生成都會追加一個新版本。', ja: '同じソースを再公開するたびに新しいバージョンが追加されます。' },
  'artifacts.timeline': { en: 'Timeline', 'zh-CN': '时间线', 'zh-TW': '時間線', ja: 'タイムライン' },
  'artifacts.highlights': { en: 'Highlights', 'zh-CN': '重点', 'zh-TW': '重點', ja: 'ハイライト' },
  'artifacts.brief': { en: 'Artifact brief', 'zh-CN': 'Artifact 摘要', 'zh-TW': 'Artifact 摘要', ja: 'Artifact概要' },
  'artifacts.toolEvidence': { en: 'Tool evidence', 'zh-CN': '工具证据', 'zh-TW': '工具證據', ja: 'ツール根拠' },
  'artifacts.codeDiff': { en: 'Code diff', 'zh-CN': '代码差异', 'zh-TW': '程式碼差異', ja: 'コード差分' },
  'artifacts.delete': { en: 'Delete artifact', 'zh-CN': '删除看板', 'zh-TW': '刪除看板', ja: 'Artifactを削除' },
  'artifacts.deleteTitle': { en: 'Delete artifact', 'zh-CN': '删除看板', 'zh-TW': '刪除看板', ja: 'Artifactを削除' },
  'artifacts.deleteConfirm': { en: 'Delete artifact “{0}”? This cannot be undone.', 'zh-CN': '删除看板“{0}”？此操作无法撤销。', 'zh-TW': '刪除看板「{0}」？此操作無法復原。', ja: 'Artifact「{0}」を削除しますか？元に戻せません。' },
  'artifacts.deleted': { en: 'Artifact deleted', 'zh-CN': '看板已删除', 'zh-TW': '看板已刪除', ja: 'Artifactを削除しました' },
  'artifacts.sourceSession': { en: 'Source session', 'zh-CN': '来源会话', 'zh-TW': '來源會話', ja: 'ソースセッション' },

  // Status messages
  'status.skillsLoaded': { en: 'Skills loaded', 'zh-CN': '技能已加载', 'zh-TW': '技能已載入', ja: 'スキル読み込み完了' },
  'status.modelsLoaded': { en: 'Models loaded', 'zh-CN': '模型已加载', 'zh-TW': '模型已載入', ja: 'モデル読み込み完了' },
  'status.connecting': { en: 'Connecting...', 'zh-CN': '连接中...', 'zh-TW': '連線中...', ja: '接続中...' },

  // Session time
  'time.justNow': { en: 'Just now', 'zh-CN': '刚刚', 'zh-TW': '剛剛', ja: 'たった今' },

  // Misc
  'misc.messages': { en: 'messages', 'zh-CN': '条消息', 'zh-TW': '條訊息', ja: 'メッセージ' },

  // New messages
  'chat.newMessages': { en: 'New messages', 'zh-CN': '新消息', 'zh-TW': '新訊息', ja: '新しいメッセージ' },
  'chat.newMessagesCount': { en: '{n} new messages', 'zh-CN': '{n} 条新消息', 'zh-TW': '{n} 條新訊息', ja: '{n}件の新しいメッセージ' },
  'chat.newMessageCount': { en: '1 new message', 'zh-CN': '1 条新消息', 'zh-TW': '1 條新訊息', ja: '1件の新しいメッセージ' },

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
