import { Page, Locator } from '@playwright/test';

export class SessionList {
  readonly page: Page;
  readonly container: Locator;
  readonly sessionItems: Locator;
  readonly sessionGroups: Locator;
  readonly emptyState: Locator;

  constructor(page: Page) {
    this.page = page;
    this.container = page.locator('.session-list');
    this.sessionItems = page.locator('.session-item');
    this.sessionGroups = page.locator('.session-group');
    this.emptyState = page.locator('.empty-sessions');
  }

  async getSessionCount(): Promise<number> {
    return await this.sessionItems.count();
  }

  async getSessionByIndex(index: number): Promise<Locator> {
    return this.sessionItems.nth(index);
  }

  async getSessionByDirectory(directory: string): Promise<Locator | null> {
    const group = this.page.locator(`.session-group:has(.group-header:has-text("${directory}"))`);
    if (await group.isVisible()) {
      return group.locator('.session-item').first();
    }
    return null;
  }

  async clickSession(index: number) {
    const session = await this.getSessionByIndex(index);
    await session.click();
  }

  async getSessionTitle(index: number): Promise<string | null> {
    const session = await this.getSessionByIndex(index);
    const title = session.locator('.session-title');
    return await title.textContent();
  }

  async getSessionPreview(index: number): Promise<string | null> {
    const session = await this.getSessionByIndex(index);
    const preview = session.locator('.session-preview');
    return await preview.textContent();
  }

  async isEmptyStateVisible(): Promise<boolean> {
    return await this.emptyState.isVisible();
  }

  async waitForSessionToAppear(timeout: number = 10000) {
    await this.sessionItems.first().waitFor({ state: 'visible', timeout });
  }

  async getGroupCount(): Promise<number> {
    return await this.sessionGroups.count();
  }

  async expandGroup(directory: string) {
    const groupHeader = this.page.locator(`.group-header:has-text("${directory}")`);
    const chevron = groupHeader.locator('.chevron');
    const isCollapsed = await chevron.evaluate(el => 
      el.classList.contains('collapsed') || el.textContent?.includes('▶')
    );
    if (isCollapsed) {
      await groupHeader.click();
    }
  }

  async collapseGroup(directory: string) {
    const groupHeader = this.page.locator(`.group-header:has-text("${directory}")`);
    const chevron = groupHeader.locator('.chevron');
    const isExpanded = await chevron.evaluate(el => 
      !el.classList.contains('collapsed') || el.textContent?.includes('▼')
    );
    if (isExpanded) {
      await groupHeader.click();
    }
  }
}