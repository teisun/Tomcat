package com.tomcat.service.impl;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;
import com.tomcat.domain.JwtUser;
import com.tomcat.domain.User;
import com.tomcat.domain.UserRepository;
import com.tomcat.service.UserService;
import com.tomcat.utils.JwtUtil;
import com.tomcat.utils.SecurityUtils;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.authentication.BadCredentialsException;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.core.userdetails.UserDetailsService;
import org.springframework.stereotype.Service;

import java.util.Optional;

@Service
public class UserServiceImpl implements UserService {

  @Autowired
  private UserRepository userRepository;

  @Autowired
  JwtUtil jwtUtil;

  @Autowired
  SecurityUtils securityUtils;

  @Autowired
  UserDetailsService userDetailsService;



//  @Bean
//  public LocalSessionFactoryBean entityManagerFactory() {
//    LocalSessionFactoryBean sessionFactory = new LocalSessionFactoryBean();
//
//    return sessionFactory;
//  }


  @Override
  public AuthenticationResponse register(AuthenticationRequest request) {

    // 密码编码
    String encodedPassword = securityUtils.encode(request.password);
    
    User user = new User();
    user.setUsername(request.username);
    user.setPassword(encodedPassword);
    user.setEmail(request.email);
    user.setPhoneNum(request.phoneNum);
    user.addDeviceId(request.deviceId);

    User userSaved = userRepository.save(user);
    String jwtToken = jwtUtil.generateToken(userSaved.getId(), userSaved.getUsername());

    return new AuthenticationResponse(jwtToken);

  }

  @Override
  public AuthenticationResponse login(AuthenticationRequest request) {
    User user = userRepository.findByPhoneNumOrEmail(request.phoneNum, request.email)
        .orElseThrow(() -> new RuntimeException("用户不存在"));

    // 校验密码
    if (!securityUtils.matches(request.password, user.getPassword())) {
      throw new BadCredentialsException("密码不正确");
    }

    JwtUser userDetails = new JwtUser(user);
    UsernamePasswordAuthenticationToken authentication = new UsernamePasswordAuthenticationToken(userDetails, null, userDetails.getAuthorities());
    SecurityContextHolder.getContext().setAuthentication(authentication);

    // 返回JWT令牌
    String jwtToken = jwtUtil.generateToken(user.getId(), user.getUsername());
    return new AuthenticationResponse(jwtToken);
  }

  @Override
  public Optional<User> findByUsername(String username) {
    return userRepository.findByUsername(username);
  }

  @Override
  public AuthenticationResponse registerOrLogin(AuthenticationRequest request) {
      AuthenticationResponse response;
    Optional<User> userOptional = userRepository.findByUsername(request.username);

    if(!userOptional.isPresent()){
        if (request.username == null){
          request.username = request.deviceId;
        }
        response = register(request);
      }else {
        response = login(request);
      }
      return response;
  }

}