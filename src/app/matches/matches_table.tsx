'use client'

import {invoke} from "@tauri-apps/api/tauri";
import {useState, useEffect} from "react";
import Link from "next/link";

interface MTGAMatch {
    id: number;
    controller_player_name: string;
    opponent_player_name: string;
    created_at: string;
}

export default function MatchesTable() {
    const [matches, setMatches] = useState<MTGAMatch[]>([]);

    useEffect(() => {
        invoke<MTGAMatch[]>('get_matches', {})
            .then(result => {
                setMatches(result);
            })
            .catch(console.error)
    }, [])

    return (
        <table>
            <thead>
            <tr>
                <th>Match ID</th>
                <th>Controller</th>
                <th>Opponent</th>
                <th>Created At</th>
            </tr>
            </thead>
            <tbody>
                {matches.map((match, index) => (
                    <tr key={index}>
                        <td><Link href={"/match-details?id=" + match.id}>{match.id}</Link></td>
                        <td>{match.controller_player_name}</td>
                        <td>{match.opponent_player_name}</td>
                        <td>{match.created_at}</td>
                    </tr>
                ))}
            </tbody>
        </table>
    );
}