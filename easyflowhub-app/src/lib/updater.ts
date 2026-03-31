/**
 * EasyFlowHub 更新检查工具
 * 使用 Tauri Updater 插件检查和安装更新
 */

import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

/**
 * 检查更新
 * 返回更新信息，如果正在下载则返回 null
 */
export async function checkForUpdates(): Promise<UpdateInfo | null> {
  try {
    const update = await check();

    if (!update) {
      return { available: false };
    }

    return {
      available: true,
      version: update.version,
      notes: update.body || undefined,
      date: update.date || undefined,
    };
  } catch (error) {
    console.error("检查更新失败:", error);
    return null;
  }
}

/**
 * 下载并安装更新
 * @param onProgress 进度回调 (downloaded: number, total: number)
 */
export async function downloadAndInstall(
  onProgress?: (downloaded: number, total: number) => void
): Promise<boolean> {
  try {
    const update = await check();

    if (!update) {
      return false;
    }

    // 下载更新
    await update.downloadAndInstall((event) => {
      if (event.event === "Progress") {
        const progress = event.data;
        // 调用进度回调
        onProgress?.(progress.chunkLength, 0);
      }
    });

    // 重新启动应用以应用更新
    await relaunch();
    return true;
  } catch (error) {
    console.error("安装更新失败:", error);
    return false;
  }
}

/**
 * 静默检查更新（仅检查，不提示用户）
 * 适用于启动时后台检查
 */
export async function silentUpdateCheck(): Promise<void> {
  try {
    const update = await check();
    if (update) {
      console.log(`发现新版本: ${update.version}`);
      // 可以选择在这里下载，或者等待用户确认
      // await update.downloadAndInstall();
    }
  } catch {
    // 静默忽略错误
  }
}
