package io.github.fffonion.yahu;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Intent;
import android.content.pm.ApplicationInfo;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ProgressBar;
import android.widget.Toast;
import java.util.ArrayList;
import org.mozilla.geckoview.AllowOrDeny;
import org.mozilla.geckoview.GeckoResult;
import org.mozilla.geckoview.GeckoRuntime;
import org.mozilla.geckoview.GeckoRuntimeSettings;
import org.mozilla.geckoview.GeckoSession;
import org.mozilla.geckoview.GeckoSessionSettings;
import org.mozilla.geckoview.GeckoView;
import org.mozilla.geckoview.WebRequestError;

public final class MainActivity extends Activity {
    private static final int FILE_CHOOSER = 10;
    private static GeckoRuntime runtime;
    private GeckoView browser;
    private GeckoSession session;
    private ProgressBar progress;
    private String serverUrl;
    private boolean showingNetworkError;
    private boolean fullScreen;
    private boolean canGoBack;
    private ChoicePromptController choicePrompts;
    private GeckoResult<GeckoSession.PromptDelegate.PromptResponse> fileResult;
    private GeckoSession.PromptDelegate.FilePrompt filePrompt;

    @Override
    public void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        getWindow().setStatusBarColor(Color.rgb(23, 23, 23));
        getWindow().setNavigationBarColor(Color.rgb(23, 23, 23));
        serverUrl = getPreferences(MODE_PRIVATE).getString("server_url", getString(R.string.server_url));
        if (!NavigationPolicy.isWebUrl(serverUrl)) serverUrl = getString(R.string.server_url);
        FrameLayout root = new FrameLayout(this);
        browser = new GeckoView(this);
        root.addView(browser, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        progress = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        progress.setMax(100);
        root.addView(progress, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, (int) (3 * getResources().getDisplayMetrics().density)));
        setContentView(root);
        applySystemBars();
        choicePrompts = new ChoicePromptController(this);

        if (runtime == null) {
            boolean debug = (getApplicationInfo().flags & ApplicationInfo.FLAG_DEBUGGABLE) != 0;
            runtime = GeckoRuntime.create(getApplicationContext(), new GeckoRuntimeSettings.Builder()
                    .remoteDebuggingEnabled(debug).build());
        }
        session = new GeckoSession(new GeckoSessionSettings.Builder()
                .usePrivateMode(false)
                .userAgentMode(GeckoSessionSettings.USER_AGENT_MODE_MOBILE)
                .build());
        session.setProgressDelegate(new GeckoSession.ProgressDelegate() {
            @Override
            public void onPageStart(GeckoSession current, String url) {
                progress.setVisibility(View.VISIBLE);
            }

            @Override
            public void onProgressChange(GeckoSession current, int value) {
                progress.setProgress(value);
            }

            @Override
            public void onPageStop(GeckoSession current, boolean success) {
                progress.setVisibility(View.GONE);
                Log.i("YahuShell", "page_loaded=" + success);
            }
        });
        session.setContentDelegate(new GeckoSession.ContentDelegate() {
            @Override
            public void onFullScreen(GeckoSession current, boolean enabled) {
                fullScreen = enabled;
                applySystemBars();
                Log.i("YahuShell", "fullscreen=" + enabled);
            }

            @Override
            public void onCrash(GeckoSession current) {
                showConnectionError();
            }
        });
        session.setNavigationDelegate(new GeckoSession.NavigationDelegate() {
            @Override
            public GeckoResult<AllowOrDeny> onLoadRequest(GeckoSession current, LoadRequest request) {
                if (handleNavigation(request.uri)) return GeckoResult.fromValue(AllowOrDeny.DENY);
                if (request.target == TARGET_WINDOW_NEW) {
                    current.loadUri(request.uri);
                    return GeckoResult.fromValue(AllowOrDeny.DENY);
                }
                return GeckoResult.fromValue(AllowOrDeny.ALLOW);
            }

            @Override
            public void onCanGoBack(GeckoSession current, boolean available) {
                canGoBack = available;
            }

            @Override
            public GeckoResult<String> onLoadError(GeckoSession current, String uri, WebRequestError error) {
                showConnectionError();
                return null;
            }
        });
        session.setPromptDelegate(new GeckoSession.PromptDelegate() {
            @Override
            public GeckoResult<PromptResponse> onChoicePrompt(GeckoSession current, ChoicePrompt prompt) {
                return choicePrompts.show(prompt);
            }

            @Override
            public GeckoResult<PromptResponse> onFilePrompt(GeckoSession current, FilePrompt prompt) {
                cancelFilePrompt();
                filePrompt = prompt;
                fileResult = new GeckoResult<>();
                GeckoResult<PromptResponse> result = fileResult;
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                        .addCategory(Intent.CATEGORY_OPENABLE).setType("*/*")
                        .putExtra(Intent.EXTRA_ALLOW_MULTIPLE, prompt.type == FilePrompt.Type.MULTIPLE);
                if (prompt.mimeTypes != null && prompt.mimeTypes.length > 0) intent.putExtra(Intent.EXTRA_MIME_TYPES, prompt.mimeTypes);
                try {
                    startActivityForResult(intent, FILE_CHOOSER);
                } catch (ActivityNotFoundException error) {
                    cancelFilePrompt();
                    Toast.makeText(MainActivity.this, "找不到文件选择器", Toast.LENGTH_SHORT).show();
                }
                return result;
            }
        });
        session.open(runtime);
        browser.setSession(session);
        session.setActive(true);
        session.loadUri(serverUrl);
        session.getUserAgent().accept(ua -> Log.i("YahuShell", "engine_ua=" + ua));
    }

    private boolean handleNavigation(String url) {
        if (NavigationPolicy.sameOrigin(serverUrl, url) || "about:blank".equals(url)) return false;
        if (NavigationPolicy.isWebUrl(url)) {
            try {
                startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
            } catch (ActivityNotFoundException error) {
                Toast.makeText(this, "找不到浏览器", Toast.LENGTH_SHORT).show();
            }
        }
        return true;
    }

    private void showConnectionError() {
        if (showingNetworkError || isFinishing()) return;
        showingNetworkError = true;
        new AlertDialog.Builder(this)
                .setTitle("无法连接 Yahu")
                .setMessage("请检查网络连接或服务器地址。")
                .setPositiveButton("重试", (dialog, which) -> session.loadUri(serverUrl))
                .setNeutralButton("服务器地址", (dialog, which) -> editServerUrl())
                .setNegativeButton("关闭", null)
                .setOnDismissListener(dialog -> showingNetworkError = false)
                .show();
    }

    private void editServerUrl() {
        EditText input = new EditText(this);
        input.setSingleLine(true);
        input.setInputType(android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_URI);
        input.setText(serverUrl);
        AlertDialog dialog = new AlertDialog.Builder(this)
                .setTitle("服务器地址")
                .setView(input)
                .setPositiveButton("打开", null)
                .setNegativeButton("取消", null)
                .create();
        dialog.setOnShowListener(ignored -> dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(view -> {
            String url = input.getText().toString().trim();
            if (!NavigationPolicy.isWebUrl(url)) {
                input.setError("请输入有效的 HTTP 或 HTTPS 地址");
                return;
            }
            serverUrl = url;
            getPreferences(MODE_PRIVATE).edit().putString("server_url", serverUrl).apply();
            session.purgeHistory();
            session.loadUri(serverUrl);
            dialog.dismiss();
        }));
        dialog.show();
    }

    private void applySystemBars() {
        int flags = View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY;
        if (fullScreen) flags |= View.SYSTEM_UI_FLAG_FULLSCREEN;
        getWindow().getDecorView().setSystemUiVisibility(flags);
    }

    @Override
    public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused) applySystemBars();
        if (session != null) session.setFocused(focused);
    }

    @Override
    public void onBackPressed() {
        if (fullScreen) session.exitFullScreen();
        else if (canGoBack) session.goBack();
        else super.onBackPressed();
    }

    private void cancelFilePrompt() {
        if (fileResult != null && filePrompt != null) fileResult.complete(filePrompt.dismiss());
        fileResult = null;
        filePrompt = null;
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != FILE_CHOOSER || fileResult == null || filePrompt == null) return;
        if (resultCode != RESULT_OK || data == null) {
            cancelFilePrompt();
            return;
        }
        ArrayList<Uri> uris = new ArrayList<>();
        ClipData clips = data.getClipData();
        if (clips != null) {
            for (int i = 0; i < clips.getItemCount(); i++) uris.add(clips.getItemAt(i).getUri());
        } else if (data.getData() != null) uris.add(data.getData());
        if (uris.isEmpty()) {
            cancelFilePrompt();
            return;
        }
        fileResult.complete(filePrompt.confirm(this, uris.toArray(new Uri[0])));
        fileResult = null;
        filePrompt = null;
    }

    @Override
    protected void onPause() {
        if (session != null) session.setActive(false);
        super.onPause();
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (session != null) session.setActive(true);
    }

    @Override
    protected void onDestroy() {
        cancelFilePrompt();
        if (choicePrompts != null) choicePrompts.dismiss();
        if (session != null) session.close();
        super.onDestroy();
    }
}
