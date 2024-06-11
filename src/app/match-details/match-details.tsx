'use client'

import { invoke} from "@tauri-apps/api/tauri";
import {useEffect, useState} from "react";

interface MatchDetails {
    id: number;
    did_controller_win: boolean;
    controller_player_name: string;
    opponent_player_name: string;
}

export default function MatchDetails() {
    const [match, setMatch] = useState<MatchDetails | null>(null);
    useEffect(() => {
        let params = new URLSearchParams(document.location.search);
        let id = params.get("id");
        if (id !== null) {
            invoke<MatchDetails>('get_match_details', {matchId: id})
                .then(result => {
                    console.log(result)
                    setMatch(result);
                })
                .catch(console.error)
        } else {
            setMatch({
                id: 0,
                did_controller_win: false,
                controller_player_name: "",
                opponent_player_name: ""
            });
        }
    })

    return (
        <div>
            <h1>Match Details</h1>
            {match && (
                <div>
                    <p>Match ID: {match.id}</p>
                    <p>Controller: {match.controller_player_name}</p>
                    <p>Opponent: {match.opponent_player_name}</p>
                    <p>Winner: {match.did_controller_win ? match.controller_player_name : match.opponent_player_name}</p>
                </div>
            )}
        </div>
    )
}