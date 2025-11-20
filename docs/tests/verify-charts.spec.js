import { test, expect } from '@playwright/test';

test.describe('ChartML Plugin Auto-Registration', () => {
  test('should render pie chart with auto-registered plugin', async ({ page }) => {
    // Navigate to examples page
    await page.goto('http://localhost:5173/examples.html');

    // Wait for page to load
    await page.waitForLoadState('networkidle');

    // Check console for auto-registration messages
    const logs = [];
    page.on('console', msg => {
      if (msg.text().includes('Auto-registered')) {
        logs.push(msg.text());
      }
    });

    // Wait a bit for charts to render
    await page.waitForTimeout(2000);

    // Check if pie chart SVG is rendered
    const pieChartSvg = await page.locator('svg').first();
    await expect(pieChartSvg).toBeVisible();

    // Log what we found
    console.log('Auto-registration logs:', logs);

    // Take a screenshot for verification
    await page.screenshot({ path: 'test-results/charts-rendered.png', fullPage: true });
  });

  test('should render scatter plot with auto-registered plugin', async ({ page }) => {
    await page.goto('http://localhost:5173/examples.html');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2000);

    // Look for multiple SVGs (should have both pie and scatter)
    const svgs = await page.locator('svg').all();
    console.log(`Found ${svgs.length} SVG elements`);

    expect(svgs.length).toBeGreaterThan(0);
  });
});
