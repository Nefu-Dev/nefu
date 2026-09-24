/**
 * Nefu preload script example
 * 
 * preload.js runs before the page loads to initialize the bridge API,
 * inject global functions, set up event listeners, etc.
 */

(function () {
    'use strict';

    // ============================================================
    // Nefu bridge object
    // ============================================================
    
    /**
     * nefu global object — the bridge between frontend and backend
     * Injected automatically by the WebView in the Nefu runtime;
     * provides a fallback stub in ordinary browsers to avoid errors.
     */
    if (typeof window.nefu === 'undefined') {
        window.nefu = {
            /**
             * Call a backend method
             * @param {string} method - Method name
             * @param {...any} args - Argument list
             * @returns {Promise<any>} Backend return value
             */
            invoke: function (method) {
                var args = Array.prototype.slice.call(arguments, 1);
                console.log('[nefu.invoke]', method, args);
                // Return a mock response in non-Nefu environments
                return Promise.resolve({
                    _mock: true,
                    method: method,
                    args: args
                });
            },

            /**
             * Listen for backend events
             * @param {string} event - Event name
             * @param {Function} callback - Callback function
             */
            on: function (event, callback) {
                console.log('[nefu.on] registering event:', event);
                if (!this._listeners) this._listeners = {};
                if (!this._listeners[event]) this._listeners[event] = [];
                this._listeners[event].push(callback);
            },

            /**
             * Send a message to the backend
             * @param {string} channel - Channel name
             * @param {any} data - Message data
             */
            send: function (channel, data) {
                console.log('[nefu.send]', channel, data);
            },

            /**
             * Remove an event listener
             * @param {string} event - Event name
             * @param {Function} callback - The callback to remove
             */
            off: function (event, callback) {
                if (!this._listeners || !this._listeners[event]) return;
                if (callback) {
                    this._listeners[event] = this._listeners[event].filter(
                        function (cb) { return cb !== callback; }
                    );
                } else {
                    delete this._listeners[event];
                }
            },

            /**
             * Get application info
             * @returns {Promise<Object>} Application metadata
             */
            getAppInfo: function () {
                return this.invoke('getAppInfo');
            },

            /**
             * Show a native dialog
             * @param {string} message - Message content
             * @param {string} [title] - Title
             */
            alert: function (message, title) {
                return this.invoke('dialog.alert', message, title || 'Notice');
            },

            /**
             * Show a native confirmation dialog
             * @param {string} message - Message content
             * @param {string} [title] - Title
             * @returns {Promise<boolean>}
             */
            confirm: function (message, title) {
                return this.invoke('dialog.confirm', message, title || 'Confirm');
            },

            /**
             * Open the file selection dialog
             * @param {Object} options - Options
             * @returns {Promise<string[]>} Selected file paths
             */
            openFile: function (options) {
                return this.invoke('dialog.openFile', options || {});
            },

            /**
             * Save file dialog
             * @param {Object} options - Options
             * @returns {Promise<string>} Save path
             */
            saveFile: function (options) {
                return this.invoke('dialog.saveFile', options || {});
            },

            /**
             * Read/write local storage (persistent)
             */
            store: {
                get: function (key) {
                    return window.nefu.invoke('store.get', key);
                },
                set: function (key, value) {
                    return window.nefu.invoke('store.set', key, value);
                },
                remove: function (key) {
                    return window.nefu.invoke('store.remove', key);
                }
            },

            /**
             * Window operations
             */
            window: {
                minimize: function () {
                    return window.nefu.invoke('window.minimize');
                },
                maximize: function () {
                    return window.nefu.invoke('window.maximize');
                },
                close: function () {
                    return window.nefu.invoke('window.close');
                },
                setTitle: function (title) {
                    return window.nefu.invoke('window.setTitle', title);
                },
                setSize: function (width, height) {
                    return window.nefu.invoke('window.setSize', width, height);
                },
                setFullscreen: function (enabled) {
                    return window.nefu.invoke('window.setFullscreen', enabled);
                }
            },

            // Internal: trigger registered events
            _emit: function (event, data) {
                if (!this._listeners || !this._listeners[event]) return;
                this._listeners[event].forEach(function (cb) {
                    try { cb(data); } catch (e) { console.error(e); }
                });
            },

            // Version marker
            version: '1.0.0',
            _isNefuRuntime: false
        };
    }

    // ============================================================
    // lj() shortcut bridge function
    // ============================================================

    /**
     * lj() — lightweight bridge call shortcut
     * Equivalent to nefu.invoke(), but with simpler syntax
     * 
     * Usage:
     *   lj('methodName', arg1, arg2)
     *   await lj('getData', { id: 1 })
     */
    if (typeof window.lj === 'undefined') {
        window.lj = function () {
            return window.nefu.invoke.apply(window.nefu, arguments);
        };
    }

    // ============================================================
    // Initialization logic
    // ============================================================

    // Send a ready signal after the page finishes loading
    window.addEventListener('DOMContentLoaded', function () {
        console.log('[preload] DOM ready, notifying backend');
        nefu.send('webview.ready', {
            timestamp: Date.now(),
            url: window.location.href
        });
    });

    // Intercept unhandled Promise rejections
    window.addEventListener('unhandledrejection', function (event) {
        console.error('[preload] Unhandled Promise rejection:', event.reason);
        nefu.send('webview.error', {
            type: 'unhandledrejection',
            message: String(event.reason),
            stack: event.reason && event.reason.stack
        });
    });

    // Intercept global JS errors
    window.addEventListener('error', function (event) {
        console.error('[preload] Global error:', event.message);
        nefu.send('webview.error', {
            type: 'error',
            message: event.message,
            filename: event.filename,
            lineno: event.lineno,
            colno: event.colno
        });
    });

    console.log('[preload] Nefu preload script executed');
})();
