'use client'

import {invoke} from "@tauri-apps/api/core";

export default function ResetButton() {
    let handleReset = () => {
        invoke('clear_scryfall_cache', {})
            .catch(console.error)
    }

    return (
        <button onClick={() => handleReset()}>Reset Scryfall Cache</button>
    );

}