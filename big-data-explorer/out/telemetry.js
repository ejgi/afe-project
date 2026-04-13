"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.telemetry = void 0;
const supabase_js_1 = require("@supabase/supabase-js");
// CONFIGURACIÓN GLOBAL DE TELEMETRÍA ZEN-ENGINE
// Estos datos conectan la extensión con el servidor central de telemetría.
const SUPABASE_URL = 'https://ewbtpmhbcelkosbwuqzd.supabase.co';
const SUPABASE_KEY = 'sb_publishable_uJhMXqNhEZTmsai9Dl1cAw_tQwrAUtX';
class TelemetryService {
    client = null;
    anonymousId = 'anon-' + Math.random().toString(36).substring(2, 15);
    constructor() {
        if (SUPABASE_URL && SUPABASE_KEY) {
            this.client = (0, supabase_js_1.createClient)(SUPABASE_URL, SUPABASE_KEY);
        }
    }
    async report(event) {
        if (!this.client) {
            console.log('Zen-Telemetry (Local-Only):', event);
            return;
        }
        try {
            const { error } = await this.client
                .from('zen_performance_logs')
                .insert([{ ...event, anonymous_id: this.anonymousId }]);
            if (error) {
                console.error('Zen-Telemetry Cloud Error:', error.message);
            }
            else {
                console.log('Zen-Telemetry Sync: Cloud-Success ✅');
            }
        }
        catch (e) {
            console.error('Zen-Telemetry Network Error:', e);
        }
    }
}
exports.telemetry = new TelemetryService();
//# sourceMappingURL=telemetry.js.map