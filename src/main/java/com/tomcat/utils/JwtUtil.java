package com.tomcat.utils;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.SignatureAlgorithm;

import java.util.Date;
/**
 * jwt的工具类
 */
public class JwtUtil {
    // 过期时间为3分钟
    private static final long EXPIRE_TIME = 365 * 24 * 60 * 60 * 1000;
    // 秘钥
    private static final String TOKEN_SECRET_KEY = "jxxxooo9999";
    /**
     * 创建token
     *
     * @param userId
     * @param username
     * @return
     */
    public static String sign(long userId, String username) {
        Date expire_date = new Date(System.currentTimeMillis() + EXPIRE_TIME);
        return Jwts.builder().setSubject(username)
                .claim("userId", userId)
                .claim("username", username)
                .setIssuedAt(new Date())
                .setExpiration(expire_date)
                .signWith(SignatureAlgorithm.HS256, TOKEN_SECRET_KEY)
                .compact();
    }
    /**
     * 验证token
     *
     * @param token
     * @return
     */
    public static Claims getClaims(String token) {
        return Jwts.parser().setSigningKey(TOKEN_SECRET_KEY).parseClaimsJws(token).getBody();
    }
}