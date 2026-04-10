export const MANAGER_NAVIGATE_TO_EXTENSION_EVENT = 'easyflowhub:navigate-to-extension';
export const MANAGER_OPEN_EXTENSION_EVENT = 'easyflowhub:open-extension';

export interface ManagerExtensionNavigationDetail {
  extensionId: string;
}

export function navigateToManagerExtension(extensionId: string): void {
  window.dispatchEvent(
    new CustomEvent<ManagerExtensionNavigationDetail>(MANAGER_NAVIGATE_TO_EXTENSION_EVENT, {
      detail: { extensionId },
    })
  );
}
