import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['src/**/*.test.ts'],
    exclude: ['copy/**', 'dist/**', 'node_modules/**', 'src-tauri/**']
  }
});
