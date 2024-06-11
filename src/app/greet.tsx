'use client'

import { useEffect, useState } from "react";
import { invoke } from '@tauri-apps/api/tauri';

export default function Hello() {
    const [greeting, setGreeting] = useState('');

    useEffect(() => {
        invoke<string>('hello_next_tauri', {})
            .then(result => setGreeting(result))
            .catch(console.error)
    }, [])
    
    return <div>{greeting}</div>;
}
