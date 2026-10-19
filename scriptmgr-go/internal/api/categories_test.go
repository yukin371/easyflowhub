package api

import (
	"testing"

	"scriptmgr/internal/model"
)

// F12: 分类计数/过滤/详情必须与列表走同一个 override-aware 视图。
// 旧实现 ListCategories/ListScriptsByCategory 直接查 discovery，设置
// override 后计数仍落在旧分类，界面各处分类不一致。
func TestCategoryOverrideAppliesToCountsAndFilters(t *testing.T) {
	instance, root := newScriptFSTestAPI(t)

	record, err := instance.CreateScript(model.CreateScriptRequest{
		Root:     root,
		Filename: "alpha.py",
		Content:  "print(1)\n",
		Category: "original",
	})
	if err != nil {
		t.Fatalf("create script: %v", err)
	}

	before, err := instance.ListCategories()
	if err != nil {
		t.Fatalf("list categories: %v", err)
	}
	if before["original"] != 1 {
		t.Fatalf("expected 1 script in original, got %v", before)
	}

	if err := instance.UpdateScriptCategory(record.ID, "reclassified"); err != nil {
		t.Fatalf("update category: %v", err)
	}

	// Counts follow the override.
	after, err := instance.ListCategories()
	if err != nil {
		t.Fatalf("list categories after override: %v", err)
	}
	if after["reclassified"] != 1 {
		t.Fatalf("counts must use the overridden category, got %v", after)
	}
	if after["original"] != 0 {
		t.Fatalf("old category must be empty after override, got %v", after)
	}

	// Filtering follows the override.
	filtered, err := instance.ListScriptsByCategory("reclassified")
	if err != nil {
		t.Fatalf("list by category: %v", err)
	}
	if len(filtered) != 1 || filtered[0].ID != record.ID {
		t.Fatalf("filter must use the overridden category, got %+v", filtered)
	}
	empty, err := instance.ListScriptsByCategory("original")
	if err != nil {
		t.Fatalf("list by old category: %v", err)
	}
	if len(empty) != 0 {
		t.Fatalf("old category filter must be empty, got %+v", empty)
	}

	// The list endpoint agrees.
	scripts, _, err := instance.ListScripts("")
	if err != nil {
		t.Fatalf("list scripts: %v", err)
	}
	found := false
	for _, script := range scripts {
		if script.ID == record.ID {
			found = true
			if script.Category != "reclassified" {
				t.Fatalf("list must report overridden category, got %q", script.Category)
			}
		}
	}
	if !found {
		t.Fatal("script missing from list")
	}

	// Clearing the override restores the discovered category everywhere.
	if err := instance.UpdateScriptCategory(record.ID, ""); err != nil {
		t.Fatalf("clear override: %v", err)
	}
	cleared, err := instance.ListCategories()
	if err != nil {
		t.Fatalf("list categories after clear: %v", err)
	}
	if cleared["original"] != 1 || cleared["reclassified"] != 0 {
		t.Fatalf("clearing override must restore discovered category, got %v", cleared)
	}
}
