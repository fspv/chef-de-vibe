import { Page, Locator } from '@playwright/test';

export class ChatWindow {
  readonly page: Page;
  readonly container: Locator;
  readonly messageList: Locator;
  readonly messageInput: Locator;
  readonly sendButton: Locator;
  readonly messages: Locator;
  readonly loadingIndicator: Locator;

  constructor(page: Page) {
    this.page = page;
    this.container = page.locator('.chat-window');
    this.messageList = page.locator('.message-list');
    this.messageInput = page.locator('textarea[placeholder*="Type your message"]');
    this.sendButton = page.locator('button:has-text("Send")');
    this.messages = page.locator('.message-bubble');
    this.loadingIndicator = page.locator('.loading-dots');
  }

  async sendMessage(message: string) {
    await this.messageInput.fill(message);
    await this.sendButton.click();
  }

  async getMessageCount(): Promise<number> {
    return await this.messages.count();
  }

  async getMessageByIndex(index: number): Promise<Locator> {
    return this.messages.nth(index);
  }

  async getMessageText(index: number): Promise<string | null> {
    const message = await this.getMessageByIndex(index);
    return await message.textContent();
  }

  async waitForResponse(timeout: number = 30000) {
    await this.loadingIndicator.waitFor({ state: 'hidden', timeout });
  }

  async isLoading(): Promise<boolean> {
    return await this.loadingIndicator.isVisible();
  }

  async getLastMessage(): Promise<string | null> {
    const count = await this.getMessageCount();
    if (count > 0) {
      return await this.getMessageText(count - 1);
    }
    return null;
  }

  async waitForMessageContaining(text: string, timeout: number = 30000) {
    await this.page.locator(`.message-bubble:has-text("${text}")`).waitFor({ 
      state: 'visible', 
      timeout 
    });
  }
}