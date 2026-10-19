package io.github.fffonion.yahu;

import android.app.Activity;
import android.app.AlertDialog;
import android.view.View;
import android.view.ViewGroup;
import android.widget.ArrayAdapter;
import android.widget.ListView;
import java.util.ArrayList;
import org.mozilla.geckoview.GeckoResult;
import org.mozilla.geckoview.GeckoSession.PromptDelegate.ChoicePrompt;
import org.mozilla.geckoview.GeckoSession.PromptDelegate.PromptResponse;

final class ChoicePromptController {
    private final Activity activity;
    private AlertDialog dialog;

    ChoicePromptController(Activity activity) {
        this.activity = activity;
    }

    private static final class Row {
        final ChoicePrompt.Choice choice;
        final boolean enabled;

        Row(ChoicePrompt.Choice choice, boolean parentDisabled) {
            this.choice = choice;
            enabled = !parentDisabled && !choice.disabled && !choice.separator && choice.items == null;
        }

        @Override
        public String toString() {
            return choice.separator ? "—" : choice.label;
        }
    }

    private static void flatten(ChoicePrompt.Choice[] choices, boolean parentDisabled, ArrayList<Row> rows) {
        for (ChoicePrompt.Choice choice : choices) {
            rows.add(new Row(choice, parentDisabled));
            if (choice.items != null) flatten(choice.items, parentDisabled || choice.disabled, rows);
        }
    }

    GeckoResult<PromptResponse> show(ChoicePrompt prompt) {
        dismiss();
        GeckoResult<PromptResponse> result = new GeckoResult<>();
        boolean[] settled = {false};
        ArrayList<Row> rows = new ArrayList<>();
        flatten(prompt.choices, false, rows);
        boolean multiple = prompt.type == ChoicePrompt.Type.MULTIPLE;
        int layout = multiple ? android.R.layout.simple_list_item_multiple_choice : android.R.layout.simple_list_item_single_choice;
        ArrayAdapter<Row> adapter = new ArrayAdapter<Row>(activity, layout, rows) {
            @Override
            public boolean areAllItemsEnabled() { return false; }

            @Override
            public boolean isEnabled(int position) { return rows.get(position).enabled; }

            @Override
            public View getView(int position, View recycled, ViewGroup parent) {
                View view = super.getView(position, recycled, parent);
                view.setEnabled(isEnabled(position));
                return view;
            }
        };
        AlertDialog.Builder builder = new AlertDialog.Builder(activity).setNegativeButton("取消", null);
        if (prompt.title != null && !prompt.title.isEmpty()) builder.setTitle(prompt.title);
        if (multiple) {
            builder.setAdapter(adapter, null);
            builder.setPositiveButton("确定", (current, which) -> {
                ArrayList<String> ids = new ArrayList<>();
                ListView list = ((AlertDialog) current).getListView();
                for (int index = 0; index < rows.size(); index++) {
                    ChoicePrompt.Choice choice = rows.get(index).choice;
                    if (list.isItemChecked(index) && choice.items == null && !choice.separator) ids.add(choice.id);
                }
                settled[0] = true;
                result.complete(prompt.confirm(ids.toArray(new String[0])));
            });
        } else {
            int selected = -1;
            for (int index = 0; index < rows.size(); index++) {
                if (rows.get(index).choice.selected) { selected = index; break; }
            }
            builder.setSingleChoiceItems(adapter, selected, (current, which) -> {
                if (!rows.get(which).enabled || settled[0]) return;
                settled[0] = true;
                result.complete(prompt.confirm(rows.get(which).choice.id));
                current.dismiss();
            });
        }
        AlertDialog current = builder.create();
        current.setOnDismissListener(ignored -> {
            if (dialog == current) dialog = null;
            if (!settled[0]) {
                settled[0] = true;
                result.complete(prompt.dismiss());
            }
        });
        if (multiple) current.setOnShowListener(ignored -> {
            ListView list = current.getListView();
            list.setChoiceMode(ListView.CHOICE_MODE_MULTIPLE);
            for (int index = 0; index < rows.size(); index++) list.setItemChecked(index, rows.get(index).choice.selected);
        });
        dialog = current;
        current.show();
        return result;
    }

    void dismiss() {
        AlertDialog current = dialog;
        dialog = null;
        if (current != null) current.dismiss();
    }
}
