const { app, BrowserWindow, ipcMain, dialog, shell, Menu } = require('electron');
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
    icon: path.join(__dirname, 'icon.ico'),
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
      spellcheck: true // Enable built-in spell checker
    }

  });

  mainWindow.loadFile(path.join(__dirname, 'renderer', 'index.html'));

  // Configure spell checker after window loads
  mainWindow.webContents.once('did-finish-load', () => {
    // Set spell checker language (defaults to system language)
    // You can set specific languages: ['en-US', 'en-GB', 'es-ES', etc.]
    mainWindow.webContents.session.setSpellCheckerLanguages(['en-US']);
  });

  // Set up context menu with spell check suggestions
  mainWindow.webContents.on('context-menu', (event, params) => {
    const menuItems = [];

    // Add spell check suggestions if available
    if (params.misspelledWord && params.dictionarySuggestions && params.dictionarySuggestions.length > 0) {
      // Add suggestions
      params.dictionarySuggestions.forEach((suggestion) => {
        menuItems.push({
          label: suggestion,
          click: () => {
            mainWindow.webContents.replaceMisspelling(suggestion);
          }
        });
      });

      // Add separator if there are suggestions
      if (params.dictionarySuggestions.length > 0) {
        menuItems.push({ type: 'separator' });
      }
    }

    // Add "Add to Dictionary" option for misspelled words
    if (params.misspelledWord) {
      menuItems.push({
        label: 'Add to Dictionary',
        click: () => {
          mainWindow.webContents.session.addWordToSpellCheckerDictionary(params.misspelledWord);
        }
      });
      menuItems.push({ type: 'separator' });
    }

    // Standard editing options
    if (params.editFlags.canCut) {
      menuItems.push({
        label: 'Cut',
        role: 'cut',
        enabled: params.editFlags.canCut
      });
    }
    if (params.editFlags.canCopy) {
      menuItems.push({
        label: 'Copy',
        role: 'copy',
        enabled: params.editFlags.canCopy
      });
    }
    if (params.editFlags.canPaste) {
      menuItems.push({
        label: 'Paste',
        role: 'paste',
        enabled: params.editFlags.canPaste
      });
    }
    if (params.editFlags.canSelectAll) {
      menuItems.push({
        label: 'Select All',
        role: 'selectAll',
        enabled: params.editFlags.canSelectAll
      });
    }

    // Build menu from template (only if there are items)
    if (menuItems.length > 0) {
      const menu = Menu.buildFromTemplate(menuItems);
      menu.popup();
    }
  });

  // Open DevTools in development
  if (process.argv.includes('--dev')) {
    mainWindow.webContents.openDevTools();
  }
}

const { spawn } = require('child_process');

let backendProcess = null;

function startBackend() {
  const isDev = !app.isPackaged;
  let backendPath;
  
  if (isDev) {
    // In development, assume backend is in sibling directory
    backendPath = path.join(__dirname, '../../rust-backend/target/release/nlp-file-explorer-backend.exe');
  } else {
    // In production, backend is in resources/bin
    backendPath = path.join(process.resourcesPath, 'bin', 'nlp-file-explorer-backend.exe');
  }
  
  console.log('Starting backend from:', backendPath);
  
  // Check if backend exists
  fs.access(backendPath).then(() => {
     backendProcess = spawn(backendPath, [], {
      stdio: 'inherit',
      windowsHide: true
    });
    
    backendProcess.on('error', (err) => {
      console.error('Failed to start backend:', err);
    });
    
    backendProcess.on('exit', (code, signal) => {
      console.log(`Backend exited with code ${code} and signal ${signal}`);
    });
  }).catch(err => {
      console.error('Backend executable not found at:', backendPath);
      console.error('Error:', err);
  });
}

function stopBackend() {
  if (backendProcess) {
    console.log('Stopping backend...');
    backendProcess.kill();
    backendProcess = null;
  }
}

app.whenReady().then(() => {
  startBackend();
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('will-quit', () => {
  stopBackend();
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
      timeout: 90000,
      validateStatus: (status) => status < 500 // Don't throw on 4xx errors, let us handle them
    });
    
    // Check if response status indicates an error
    if (response.status >= 400) {
      // For 4xx errors, try to extract error message from response data
      const errorData = response.data;
      return {
        success: false,
        error: errorData?.error || errorData?.message || `HTTP ${response.status}: ${response.statusText}`,
        data: errorData // Include data in case backend returns structured error
      };
    }
    
    return { success: true, data: response.data };
  } catch (error) {
    // Handle network errors, timeouts, etc.
    if (error.response) {
      // Server responded with error status
      const errorData = error.response.data;
      return {
        success: false,
        error: errorData?.error || errorData?.message || `HTTP ${error.response.status}: ${error.response.statusText}`,
        data: errorData
      };
    } else if (error.request) {
      // Request made but no response received
      return {
        success: false,
        error: 'No response from server. Is the backend running?'
      };
    } else {
      // Error setting up request
      return {
        success: false,
        error: error.message || 'Unknown error occurred'
      };
    }
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

// Spell checker handlers
ipcMain.handle('get-spell-checker-languages', () => {
  if (mainWindow && mainWindow.webContents) {
    return { success: true, languages: mainWindow.webContents.session.getSpellCheckerLanguages() };
  }
  return { success: false, languages: [] };
});

ipcMain.handle('set-spell-checker-languages', (event, languages) => {
  if (mainWindow && mainWindow.webContents) {
    mainWindow.webContents.session.setSpellCheckerLanguages(languages);
    return { success: true };
  }
  return { success: false };
});

// Extract icon from .exe file (Windows only)
// Note: This is a basic implementation. For full icon extraction, consider using
// a native module like 'node-icon-extractor' or Windows Shell API bindings
ipcMain.handle('get-file-icon', async (event, filePath) => {
  try {
    // For .exe files on Windows, try to extract icon
    if (process.platform === 'win32' && filePath.toLowerCase().endsWith('.exe')) {
      // TODO: Implement actual icon extraction using native module or Windows API
      // For now, return false to use default icon
      // Example with native module:
      // const iconExtractor = require('node-icon-extractor');
      // const iconPath = await iconExtractor.extractIcon(filePath);
      // if (iconPath) {
      //   return { success: true, iconPath };
      // }
      
      return { success: false };
    }
    
    // Fallback to default icon
    return { success: false };
  } catch (error) {
    console.error('Error extracting icon:', error);
    return { success: false, error: error.message };
  }
});