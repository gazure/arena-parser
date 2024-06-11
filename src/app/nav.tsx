// Top Nav component
// import Nav from "./nav"
import Link from "next/link";

export default function Nav() {
    return (<nav className="flex items-center justify-between flex-wrap bg-amber-500 p-6">
            <div className="flex items-center flex-shrink-0 text-white mr-6">
                <Link href={"/"}><span className="font-semibold text-xl tracking-tight p-8">
                    Arena Parser
                </span></Link>
                <Link href="/matches"><span className="font-semibold text-xl tracking-tight p-8">
                    Matches
                </span></Link>
            </div>
        </nav>);
}