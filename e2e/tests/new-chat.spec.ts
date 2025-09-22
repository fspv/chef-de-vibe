import { test, expect } from '@playwright/test';
import { NewChatDialog } from '../fixtures/NewChatDialog';
import { SessionList } from '../fixtures/SessionList';
import { ChatWindow } from '../fixtures/ChatWindow';

test.describe('New Chat Creation', () => {
  test('should create session and display in session list', async ({ page }) => {
    // Navigate to the application
    await page.goto('/');
    
    // Initialize page objects
    const newChatDialog = new NewChatDialog(page);
    const sessionList = new SessionList(page);
    const chatWindow = new ChatWindow(page);
    
    // Check initial state - should show empty sessions or existing sessions
    const initialSessionCount = await sessionList.getSessionCount();
    
    // Open new chat dialog
    await newChatDialog.openDialog();
    
    // Verify dialog is visible
    expect(await newChatDialog.isVisible()).toBe(true);
    
    // Fill in the form - use /tmp which should exist in container
    await newChatDialog.fillWorkingDirectory('/tmp');
    await newChatDialog.fillMessage('Hello Claude, this is a test message');
    
    // Keep default mode (no need to change)
    
    // Submit the form
    await newChatDialog.submitForm();
    
    // Wait for redirect to the session page
    await page.waitForURL(/\/session\/[a-f0-9-]+/, { timeout: 10000 });
    
    // Dialog should be closed after redirect
    expect(await newChatDialog.isVisible()).toBe(false);
    
    // Check that a new session was added to the list
    const newSessionCount = await sessionList.getSessionCount();
    expect(newSessionCount).toBeGreaterThan(initialSessionCount);
    
    // Verify the session shows in the correct directory group
    await sessionList.expandGroup('/tmp');
    const sessionInGroup = await sessionList.getSessionByDirectory('/tmp');
    expect(sessionInGroup).toBeTruthy();
    
    // Verify we're in the chat window for this session
    const messageCount = await chatWindow.getMessageCount();
    expect(messageCount).toBeGreaterThanOrEqual(1);
    
    // The first message should be our test message
    const firstMessage = await chatWindow.getMessageText(0);
    expect(firstMessage).toContain('Hello Claude, this is a test message');
    
    // Click on the new session to verify it loads
    await sessionList.clickSession(newSessionCount - 1);
  });

  test('should allow canceling the new chat dialog', async ({ page }) => {
    await page.goto('/');
    
    const newChatDialog = new NewChatDialog(page);
    const sessionList = new SessionList(page);
    
    const initialSessionCount = await sessionList.getSessionCount();
    
    // Open and then cancel the dialog
    await newChatDialog.openDialog();
    expect(await newChatDialog.isVisible()).toBe(true);
    
    await newChatDialog.cancel();
    expect(await newChatDialog.isVisible()).toBe(false);
    
    // Verify no new session was created
    const sessionCount = await sessionList.getSessionCount();
    expect(sessionCount).toBe(initialSessionCount);
  });

  test('should validate required fields', async ({ page }) => {
    await page.goto('/');
    
    const newChatDialog = new NewChatDialog(page);
    
    await newChatDialog.openDialog();
    
    // Check that submit button is disabled without required fields
    expect(await newChatDialog.isSubmitButtonDisabled()).toBe(true);
    
    // Fill only directory, button should still be disabled
    await newChatDialog.fillWorkingDirectory('/tmp');
    expect(await newChatDialog.isSubmitButtonDisabled()).toBe(true);
    
    // Now fill message and button should be enabled
    await newChatDialog.fillMessage('Test message');
    expect(await newChatDialog.isSubmitButtonDisabled()).toBe(false);
    
    // Submit the form
    await newChatDialog.submitForm();
    
    // Wait for dialog to close - there's typically a 5s wait after successful 
    // session creation to ensure journals are created
    await page.waitForTimeout(6000);
    
    // Dialog should close after successful submission
    expect(await newChatDialog.isVisible()).toBe(false);
  });
});
