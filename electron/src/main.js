const { app, BrowserWindow, ipcMain, dialog, shell } = require('electron');
const path = require('path');
const axios = require('axios');
const fs = require('fs').promises;

const BACKEND_URL = 'http://localhost:8080';

let mainWindow;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    frame: false,
    titleBarStyle: 'hidden',
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js')
    }
  });

  mainWindow.loadFile(path.join(__dirname, 'renderer', 'index.html'));

  // Open DevTools in development
  if (process.argv.includes('--dev')) {
    mainWindow.webContents.openDevTools();
  }
}

app.whenReady().then(() => {
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// IPC handlers for backend communication
ipcMain.handle('api-request', async (event, { method, endpoint, data }) => {
  try {
    const response = await axios({
      method,
      url: `${BACKEND_URL}${endpoint}`,
      data,
      timeout: 30000
    });
    return { success: true, data: response.data };
  } catch (error) {
    return {
      success: false,
      error: error.response?.data || error.message
    };
  }
});

// Directory selection handler
ipcMain.handle('select-directory', async () => {
  const result = await dialog.showOpenDialog(mainWindow, {
    properties: ['openDirectory']
  });
  
  if (!result.canceled && result.filePaths.length > 0) {
    return { success: true, path: result.filePaths[0] };
  }
  
  return { success: false };
});

// File opening handler
ipcMain.handle('open-file', async (event, filePath) => {
  try {
    await shell.openPath(filePath);
    return { success: true };
  } catch (error) {
    return { success: false, error: error.message };
  }
});

// Show file in folder (reveal in file explorer)
ipcMain.handle('show-in-folder', async (event, filePath) => {
  try {
    shell.showItemInFolder(filePath);
    return { success: true };
  } catch (error) {
    return { success: false, error: error.message };
  }
});

// Read file preview
ipcMain.handle('read-file-preview', async (event, filePath) => {
  try {
    const stats = await fs.stat(filePath);
    const ext = path.extname(filePath).toLowerCase();
    
    // Only preview text-based files
    if (['.txt', '.md', '.js', '.ts', '.py', '.rs', '.json', '.xml', '.html', '.css', '.yaml', '.yml'].includes(ext)) {
      const content = await fs.readFile(filePath, 'utf-8');
      // Return first 500 characters
      const preview = content.substring(0, 500);
      return { success: true, preview, isText: true };
    }
    
    return { success: true, preview: `File size: ${(stats.size / 1024).toFixed(2)} KB`, isText: false };
  } catch (error) {
    return { success: false, error: error.message };
  }
});

// Window controls
ipcMain.handle('window-minimize', () => {
  if (mainWindow) mainWindow.minimize();
});

ipcMain.handle('window-maximize', () => {
  if (mainWindow) {
    if (mainWindow.isMaximized()) {
      mainWindow.unmaximize();
    } else {
      mainWindow.maximize();
    }
  }
});

ipcMain.handle('window-close', () => {
  if (mainWindow) mainWindow.close();
});
