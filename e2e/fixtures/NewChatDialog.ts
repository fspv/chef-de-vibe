import { Page, Locator } from '@playwright/test';

export class NewChatDialog {
  readonly page: Page;
  readonly newChatButton: Locator;
  readonly dialog: Locator;
  readonly directoryInput: Locator;
  readonly messageTextarea: Locator;
  readonly modeSelector: Locator;
  readonly startChatButton: Locator;
  readonly cancelButton: Locator;
  readonly errorMessage: Locator;

  constructor(page: Page) {
    this.page = page;
    this.newChatButton = page.locator('.session-list-header button:has-text("New Chat")').first();
    this.dialog = page.locator('.new-chat-dialog');
    this.directoryInput = page.locator('.new-chat-dialog .directory-picker-input');
    this.messageTextarea = page.locator('textarea[id="message-input"]');
    this.modeSelector = page.locator('.mode-selector');
    this.startChatButton = page.locator('.new-chat-dialog button[type="submit"].confirm-button');
    this.cancelButton = page.locator('button.dialog-close-button');
    this.errorMessage = page.locator('.error-message');
  }

  async openDialog() {
    await this.newChatButton.click();
    await this.dialog.waitFor({ state: 'visible' });
  }

  async fillWorkingDirectory(path: string) {
    await this.directoryInput.fill(path);
  }

  async fillMessage(message: string) {
    await this.messageTextarea.fill(message);
  }

  async selectMode(mode: 'default' | 'plan' | 'auto' | 'no-tools') {
    const modeButton = this.page.locator(`button[data-mode="${mode}"]`);
    await modeButton.click();
  }

  async submitForm() {
    await this.startChatButton.click();
  }

  async cancel() {
    await this.cancelButton.click();
  }

  async getErrorMessage(): Promise<string | null> {
    try {
      await this.errorMessage.waitFor({ state: 'visible', timeout: 5000 });
      return await this.errorMessage.textContent();
    } catch {
      return null;
    }
  }

  async isVisible(): Promise<boolean> {
    return await this.dialog.isVisible();
  }

  async isSubmitButtonDisabled(): Promise<boolean> {
    return await this.startChatButton.isDisabled();
  }
}