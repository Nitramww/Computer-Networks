import java.io.*;
import java.net.*;
import java.util.*;

/**
 * Labai paprastas serveris (dar galima pridėti klientų žurnalus bei išėjimo įvykius) 
 * 
 */
public class PokalbiuServeris {

    private static int PORTAS; // portas

    private static HashSet<String> vardai = new HashSet<String>(); 
                     // prisijungusių klientų vardai
    private static HashSet<PrintWriter> isvedimai = new HashSet<PrintWriter>();
                     // aibė išvesties srautų, susietų su klientais
    //---------------------------------------------------------------
    public static void main(String[] args) throws Exception {
        System.out.println("Serveris veikia... ");
        PORTAS = Integer.parseInt(args[0]); 
        ServerSocket serverioSoketas = new ServerSocket(PORTAS);
        try {
            while (true) {
                new Valdymas(serverioSoketas.accept()).start();
            }
        } finally {
            serverioSoketas.close();
        }
    }

    /**
     * Vidinė klasė klientinėms gyjoms valdyti
     */
    private static class Valdymas extends Thread {
        private String vardas;
        private Socket soketas;
        private BufferedReader ivestis;
        private PrintWriter isvestis;

        public Valdymas(Socket soketas) {
            this.soketas = soketas;
        }
        // visas veiksmas kaip visada - run() metode:
        public void run() {
            try {
                ivestis = new BufferedReader(new InputStreamReader(
                    soketas.getInputStream()));
                isvestis = new PrintWriter(soketas.getOutputStream(), true);
                // bandome sužinoti kliento vardą ir užregistruoti jį:
                while (true) {
                    isvestis.println("ATSIUSKVARDA"); // protokolo pranešimas
                    vardas = ivestis.readLine();
                    if (vardas == null) {
                        return;
                    }
                    synchronized (vardai) {
                        if (!vardai.contains(vardai)) {
                            vardai.add(vardas);
                            break;
                        }
                    }
                }

                isvestis.println("VARDASOK");  // protokolo pranešimas
                isvedimai.add(isvestis);

                // Gauname žinutes iš klientų, siunčiame visiems...
                while (true) {
                    String ivesta = ivestis.readLine();
                    if (ivesta == null) {
                        return;
                    }
                    
                    for (PrintWriter isvestis : isvedimai) {
                        isvestis.println("PRANESIMAS" + vardas + ": " + ivesta);
                    }
                }
            } catch (IOException e) {
                System.out.println(e);
            } finally {
                // Klientas dingo ... 
                // Uždarome soketą bei išmetame iš vardų ir išvedimų aibių:    
                if (vardas != null) {
                    vardai.remove(vardas);
                }
                if (isvestis != null) {
                    isvedimai.remove(isvestis);
                }
                try {
                    soketas.close();
                } catch (IOException e) {
                }
            }
        }
    }
}

